//! Deadline-bounded leaf-route calls and their independent public statuses.

use super::{
    deadline::{DEADLINE_EXCEEDED, DISABLED_BY_CONFIGURATION, DeadlineElapsed, Deadlines, until},
    fusion::{Fused, Hit, Route, RouteList},
    query::Query,
    request::SearchObservations,
    rerank::Reranker,
    routes::lexical,
    routes::{
        dense::{self, Embedder},
        error::RouteError,
        outcome::{IdentifierOutcome, RouteOutcome, StructuredOutcome},
        structured::search_structured,
    },
};
use crate::index::RetrievalProjectionPort;
use maestro_kernel::{
    evidence::RouteStatus,
    gateway::{ModelPort, Role, Room},
    retrieval::InventoryRequest,
    store::Database,
    telemetry::stage::Outcome,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    future::Future,
    sync::Arc,
};
use tokio::time::{self, Instant};

/// How a route or the rerank that ended with `status` ended: the reason
/// code [`DEADLINE_EXCEEDED`] is a timeout, any other an unavailability.
pub(super) fn route_outcome(status: &RouteStatus) -> Outcome {
    match status {
        RouteStatus::Ok => Outcome::Ok,
        RouteStatus::Unavailable(reason) if reason == DEADLINE_EXCEEDED => Outcome::Timeout,
        RouteStatus::Unavailable(_) => Outcome::Unavailable,
    }
}

/// Polls every independent route before returning any route's outcome.
pub(super) async fn join_route_futures<D, L, I, S>(
    dense: D,
    lexical: L,
    identifier: I,
    structured: S,
) -> (
    RouteOutcome,
    RouteOutcome,
    IdentifierOutcome,
    Option<StructuredOutcome>,
)
where
    D: Future<Output = RouteOutcome>,
    L: Future<Output = RouteOutcome>,
    I: Future<Output = IdentifierOutcome>,
    S: Future<Output = Option<StructuredOutcome>>,
{
    tokio::join!(dense, lexical, identifier, structured)
}

/// The stable refusal for Global questions outside the closed inventory grammar.
const UNSUPPORTED_INVENTORY: &str = concat!(
    "unsupported inventory; request document counts, document sets or ",
    "versions, optionally in one named set",
);

/// Executes dense search with its independent route cutoff, which starts
/// once the embedder is ready: loading its model is setup, bounded by
/// `cutoffs.routes_end`, not route time. A setup the port refuses, or
/// one still loading at that bound, leaves the route unavailable without
/// its window.
pub(super) async fn dense_outcome<P: ModelPort, R: RetrievalProjectionPort>(
    enabled: bool,
    query: &Query<'_, R>,
    embedder: Option<&Embedder<'_, P>>,
    cutoffs: &Deadlines,
) -> RouteOutcome {
    if !enabled {
        return unavailable(DISABLED_BY_CONFIGURATION);
    }
    let Some(embedder) = embedder else {
        return unavailable("no embedder card for the published generation's profile");
    };
    // A model still loading at the bound would hold the route past it, and
    // the rerank and evidence assembly need the rest of the budget.
    match until(
        cutoffs.routes_end,
        embedder.port.prepare(embedder.card, Room::Free),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            return unavailable(route_error_reason(&RouteError::EmbedderUnavailable {
                reason: error.to_string(),
            }));
        }
        Err(DeadlineElapsed) => return unavailable(DEADLINE_EXCEEDED),
    }
    let deadline = cutoffs.route_after(Instant::now());
    if Instant::now() >= deadline {
        return unavailable(DEADLINE_EXCEEDED);
    }
    match time::timeout_at(deadline, dense::search_dense(query, embedder)).await {
        Ok(Ok(hits)) => RouteOutcome {
            hits,
            status: RouteStatus::Ok,
        },
        Ok(Err(error)) => unavailable(route_error_reason(&error)),
        Err(_) => unavailable(DEADLINE_EXCEEDED),
    }
}

/// Readies the reranker's model, its one-time setup, while the routes run,
/// bounded by `cutoffs.setup`, the rerank's own cutoff: a model still
/// loading then leaves the rerank no time, so it makes no call and the
/// search keeps the fused order and its deadline. A rerank that cannot run
/// prepares no model.
pub(super) async fn prepare_reranker<P: ModelPort>(
    reranker: Option<&Reranker<'_, P>>,
    enabled: bool,
    cutoffs: &Deadlines,
) {
    let Some(reranker) =
        reranker.filter(|reranker| enabled && reranker.card.fields().role == Role::Reranker)
    else {
        return;
    };
    // A refused setup is retried by the reranking call itself, which reports
    // its precise reason before the cutoff.
    drop(
        until(
            cutoffs.setup,
            reranker.port.prepare(reranker.card, Room::Free),
        )
        .await,
    );
}

/// Executes lexical search with its independent route cutoff.
pub(super) async fn lexical_outcome<R: RetrievalProjectionPort>(
    enabled: bool,
    query: &Query<'_, R>,
    deadline: Instant,
) -> RouteOutcome {
    if !enabled {
        return unavailable(DISABLED_BY_CONFIGURATION);
    }
    if Instant::now() >= deadline {
        return unavailable(DEADLINE_EXCEEDED);
    }
    match time::timeout_at(deadline, lexical::search_bm25(query)).await {
        Ok(Ok(hits)) => RouteOutcome {
            hits,
            status: RouteStatus::Ok,
        },
        Ok(Err(error)) => unavailable(route_error_reason(&error)),
        Err(_) => unavailable(DEADLINE_EXCEEDED),
    }
}

/// Reduces a route error to a bounded public category.
pub(super) fn route_error_reason(error: &RouteError) -> &'static str {
    match error {
        RouteError::EmbedderUnavailable { .. } => "embedder unavailable",
        RouteError::ProfileMismatch { .. } => "search profile mismatch",
        RouteError::Projection(_) => "Qdrant search failed",
        RouteError::InvalidVector(_) => "invalid query vector",
        RouteError::GenerationLookup(_)
        | RouteError::UnknownGeneration { .. }
        | RouteError::UnpublishedGeneration { .. } => "search route failed",
    }
}

/// Runs a Global inventory request and degrades unsupported forms explicitly.
pub(super) async fn structured_outcome<R>(
    query: &Query<'_, R>,
    database: Arc<Database>,
    request: Result<Option<&InventoryRequest>, &str>,
    deadline: Instant,
) -> StructuredOutcome {
    match request {
        Err(reason) => StructuredOutcome {
            route: unavailable(reason),
            inventory: None,
        },
        Ok(Some(request)) => search_structured(query, database, request, deadline).await,
        Ok(None) => StructuredOutcome {
            route: unavailable(UNSUPPORTED_INVENTORY),
            inventory: None,
        },
    }
}

/// Builds one route's RRF list without changing its score or rank order;
/// with `drop_unavailable`, an unavailable route's list is empty.
pub(super) fn route_list(
    route: Route,
    outcome: &RouteOutcome,
    drop_unavailable: bool,
) -> RouteList {
    let fused = !drop_unavailable || outcome.status == RouteStatus::Ok;
    RouteList {
        route,
        hits: outcome
            .hits
            .iter()
            .filter(|_| fused)
            .map(|hit| Hit {
                chunk_id: hit.chunk_id.clone(),
                score: hit.score,
            })
            .collect(),
    }
}

/// Records one route's status and turns degradation into an explicit gap.
pub(super) fn add_route(
    routes: &mut BTreeMap<String, RouteStatus>,
    gaps: &mut Vec<String>,
    route: Route,
    status: &RouteStatus,
) {
    add_named_route(routes, gaps, route.name(), status);
}

/// Records a named route's status and its nonblank unavailability reason.
pub(super) fn add_named_route(
    routes: &mut BTreeMap<String, RouteStatus>,
    gaps: &mut Vec<String>,
    name: &str,
    status: &RouteStatus,
) {
    if let RouteStatus::Unavailable(reason) = status {
        gaps.push(format!("{name} route unavailable: {}", nonblank(reason)));
    }
    routes.insert(name.to_owned(), status.clone());
}

/// Creates a nonblank unavailable result for a failed leaf route.
fn unavailable(reason: &str) -> RouteOutcome {
    RouteOutcome {
        hits: Vec::new(),
        status: RouteStatus::Unavailable(nonblank(reason).to_owned()),
    }
}

/// Replaces a blank diagnostic with a stable public fallback.
fn nonblank(reason: &str) -> &str {
    if reason.trim().is_empty() {
        "route unavailable"
    } else {
        reason
    }
}

/// Captures each route's candidate order before fusion.
pub(super) fn route_observations(
    dense: &RouteOutcome,
    lexical: &RouteOutcome,
    identifier: &RouteOutcome,
    structured: Option<&StructuredOutcome>,
) -> SearchObservations {
    let mut route_ranks = BTreeMap::from([
        (Route::Dense, observed_chunk_ids(dense)),
        (Route::Lexical, observed_chunk_ids(lexical)),
        (Route::Identifier, observed_chunk_ids(identifier)),
    ]);
    if let Some(structured) = structured {
        route_ranks.insert(Route::Structured, observed_chunk_ids(&structured.route));
    }
    SearchObservations {
        route_ranks,
        ..SearchObservations::default()
    }
}

/// Retains candidate identities in their route-provided rank order.
pub(super) fn observed_chunk_ids(outcome: &RouteOutcome) -> Vec<String> {
    outcome
        .hits
        .iter()
        .map(|hit| hit.chunk_id.clone())
        .collect()
}

/// Retains revision identities for every fused candidate to validate kernel ownership.
pub(super) fn revisions_for_fused<'a>(
    fused: &[Fused],
    outcomes: impl Iterator<Item = &'a RouteOutcome>,
) -> HashMap<String, Vec<String>> {
    let wanted: HashSet<&str> = fused
        .iter()
        .map(|candidate| candidate.chunk_id.as_str())
        .collect();
    let mut revisions = HashMap::new();
    for outcome in outcomes {
        for hit in &outcome.hits {
            if wanted.contains(hit.chunk_id.as_str()) {
                revisions
                    .entry(hit.chunk_id.clone())
                    .or_insert_with(Vec::new)
                    .push(hit.revision_id.clone());
            }
        }
    }
    revisions
}
