//! Deadline-bounded leaf-route calls and their independent public statuses.

use super::{
    fusion::{Hit, Route, RouteList},
    query::Query,
    routes::lexical,
    routes::{
        dense::{self, Embedder},
        error::RouteError,
        outcome::{RouteOutcome, StructuredOutcome},
        structured::search_structured,
    },
};
use maestro_kernel::{
    evidence::RouteStatus, gateway::ModelPort, retrieval::InventoryRequest, store::Database,
};
use std::{collections::BTreeMap, future::Future, sync::Arc};
use tokio::time::{self, Instant};

/// Polls every independent route before returning any route's outcome.
pub(super) async fn join_route_futures<D, L, I, S>(
    dense: D,
    lexical: L,
    identifier: I,
    structured: S,
) -> (
    RouteOutcome,
    RouteOutcome,
    RouteOutcome,
    Option<StructuredOutcome>,
)
where
    D: Future<Output = RouteOutcome>,
    L: Future<Output = RouteOutcome>,
    I: Future<Output = RouteOutcome>,
    S: Future<Output = Option<StructuredOutcome>>,
{
    tokio::join!(dense, lexical, identifier, structured)
}

/// The stable refusal for Global questions outside the closed inventory grammar.
const UNSUPPORTED_INVENTORY: &str = concat!(
    "unsupported inventory; request document counts, document sets or ",
    "versions, optionally in one named set",
);

/// Executes dense search with its independent route cutoff.
pub(super) async fn dense_outcome<P: ModelPort>(
    query: &Query<'_>,
    embedder: Option<&Embedder<'_, P>>,
    deadline: Instant,
) -> RouteOutcome {
    let Some(embedder) = embedder else {
        return unavailable("no embedder card for the published generation's profile");
    };
    if Instant::now() >= deadline {
        return unavailable("route deadline elapsed");
    }
    match time::timeout_at(deadline, dense::search_dense(query, embedder)).await {
        Ok(Ok(hits)) => RouteOutcome {
            hits,
            status: RouteStatus::Ok,
        },
        Ok(Err(error)) => unavailable(route_error_reason(&error)),
        Err(_) => unavailable("route deadline elapsed"),
    }
}

/// Executes lexical search with its independent route cutoff.
pub(super) async fn lexical_outcome(query: &Query<'_>, deadline: Instant) -> RouteOutcome {
    if Instant::now() >= deadline {
        return unavailable("route deadline elapsed");
    }
    match time::timeout_at(deadline, lexical::search_bm25(query)).await {
        Ok(Ok(hits)) => RouteOutcome {
            hits,
            status: RouteStatus::Ok,
        },
        Ok(Err(error)) => unavailable(route_error_reason(&error)),
        Err(_) => unavailable("route deadline elapsed"),
    }
}

/// Reduces a route error to a bounded public category.
pub(super) fn route_error_reason(error: &RouteError) -> &'static str {
    match error {
        RouteError::EmbedderUnavailable { .. } => "embedder unavailable",
        RouteError::ProfileMismatch { .. } => "search profile mismatch",
        RouteError::Qdrant(_) => "Qdrant search failed",
        RouteError::InvalidVector(_) => "invalid query vector",
        RouteError::GenerationLookup(_)
        | RouteError::UnknownGeneration { .. }
        | RouteError::UnpublishedGeneration { .. } => "search route failed",
    }
}

/// Runs Global inventory requests and degrades unsupported forms explicitly.
pub(super) async fn structured_outcome(
    query: &Query<'_>,
    database: Arc<Database>,
    request: Option<&InventoryRequest>,
    enabled: bool,
    deadline: Instant,
) -> Option<StructuredOutcome> {
    if !enabled {
        return None;
    }
    match request {
        Some(request) => Some(search_structured(query, database, request, deadline).await),
        None => Some(StructuredOutcome {
            route: unavailable(UNSUPPORTED_INVENTORY),
            inventory: None,
        }),
    }
}

/// Builds one route's RRF list without changing its score or rank order.
pub(super) fn route_list(route: Route, outcome: &RouteOutcome) -> RouteList {
    RouteList {
        route,
        hits: outcome
            .hits
            .iter()
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
