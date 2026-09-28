//! Admission, parallel route execution, fusion and the bounded T032 handoff.

use super::{
    admission::{AdmittedSearch, admit_request, ensure_permissions},
    candidates::{self, Failure as CandidateFailure},
    deadline::{DEADLINE_EXCEEDED, DISABLED_BY_CONFIGURATION},
    fusion::{Fused, Route, RouteList},
    query::Query,
    request::{EvidenceInput, SearchContext, SearchError, SearchObservations, SearchRequest},
    rerank::{Candidate, Ranked, Reranker, rerank},
    route_execution::{
        add_named_route, add_route, dense_outcome, join_route_futures, lexical_outcome,
        prepare_reranker, route_list, structured_outcome,
    },
    routes::{
        identifier::search_identifiers_enabled,
        outcome::{RouteOutcome, StructuredOutcome},
    },
};
use crate::query::{QueryKind, Understood};
use maestro_kernel::{
    evidence::{Inventory, RouteStatus},
    gateway::ModelPort,
    telemetry::{
        span,
        stage::{Count, Outcome, Stage},
    },
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    future::Future,
    mem,
    num::NonZeroUsize,
};
use tokio::time::Instant;

/// The dense and lexical routes' bounded candidate pool.
const DENSE_LIMIT: usize = 100;
/// The single reciprocal-rank fusion pool limit.
const FUSION_POOL: usize = 120;
/// The rerank status reason when fusion gave no candidate to rerank.
pub const NO_FUSED_CANDIDATES: &str = "no fused candidates";

/// Retrieves a pinned, scoped and deadline-bounded evidence handoff for T032.
///
/// The search is traced as a `retrieval.search` stage, whose routes, fusion
/// and rerank are its child stages. The reranker's model is readied while
/// the routes run.
///
/// # Errors
///
/// Returns [`SearchError`] when admission, permissions, or candidate integrity
/// cannot be established within the request deadline.
pub async fn search<P: ModelPort>(
    context: &SearchContext<'_, P>,
    request: &SearchRequest<'_>,
) -> Result<EvidenceInput, SearchError> {
    let stage = span::search();
    let result = stage
        .instrument(async {
            let admitted = admit_request(context, request).await?;
            // Boxed: the routes' future is large, and ask nests this one.
            let (routes, ()) = tokio::join!(
                Box::pin(execute_routes(context, &admitted)),
                prepare_reranker(
                    context.reranker.as_ref(),
                    admitted.configuration.rerank_enabled,
                    &admitted.cutoffs,
                ),
            );
            finish_search(context, request, admitted, routes).await
        })
        .await;
    if let Ok(input) = &result {
        stage.collection(&input.generation.collection_id);
        stage.generation(input.generation.id);
        stage.count(Count::Candidates, input.ranked.len());
    }
    stage.finish(result.as_ref().map_or_else(search_outcome, |_| Outcome::Ok));
    result
}

/// How a search that failed with `error` ended: refused by its contract or
/// its caller's rights, out of time, or failed.
pub(super) const fn search_outcome(error: &SearchError) -> Outcome {
    match error {
        SearchError::InvalidRequest { .. }
        | SearchError::Admission(_)
        | SearchError::PermissionsChanged => Outcome::Refused,
        SearchError::AdmissionTimedOut | SearchError::PermissionCheckTimedOut => Outcome::Timeout,
        SearchError::Kernel(_) | SearchError::EvidenceLoad { .. } | SearchError::WorkerFailed => {
            Outcome::Error
        }
    }
}

/// How a route or the rerank that ended with `status` ended: the reason
/// code [`DEADLINE_EXCEEDED`] is a timeout, any other an unavailability.
pub(super) fn route_outcome(status: &RouteStatus) -> Outcome {
    match status {
        RouteStatus::Ok => Outcome::Ok,
        RouteStatus::Unavailable(reason) if reason == DEADLINE_EXCEEDED => Outcome::Timeout,
        RouteStatus::Unavailable(_) => Outcome::Unavailable,
    }
}

/// Runs `route` as the stage `stage`, which records its hits and outcome.
async fn traced_route(stage: Stage, route: impl Future<Output = RouteOutcome>) -> RouteOutcome {
    let outcome = stage.instrument(route).await;
    stage.count(Count::Candidates, outcome.hits.len());
    stage.finish(route_outcome(&outcome.status));
    outcome
}

/// The route votes, statuses and exact document inventory produced in parallel.
struct RouteResults {
    /// The one fused list bounded for T031.
    fused: Vec<Fused>,
    /// Revision IDs retained independently from fusion for kernel validation.
    expected_revisions: HashMap<String, Vec<String>>,
    /// Each route's independent availability status.
    routes: BTreeMap<String, RouteStatus>,
    /// The complete structured result, kept outside passage fusion.
    inventory: Option<Inventory>,
    /// Every degradation known before evidence expansion.
    known_gaps: Vec<String>,
    /// Scoped candidate ranks observed before fusion.
    observations: SearchObservations,
}

/// Runs the structured `route` as its stage, which records its hits and
/// outcome, when the question is `global`; else neither runs.
async fn traced_structured(
    global: bool,
    enabled: bool,
    route: impl Future<Output = StructuredOutcome>,
) -> Option<StructuredOutcome> {
    if !global {
        return None;
    }
    let stage = span::route_structured();
    let outcome = if enabled {
        stage.instrument(route).await
    } else {
        StructuredOutcome {
            route: RouteOutcome {
                hits: Vec::new(),
                status: RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned()),
            },
            inventory: None,
        }
    };
    stage.count(Count::Candidates, outcome.route.hits.len());
    stage.finish(route_outcome(&outcome.route.status));
    Some(outcome)
}

/// Polls all applicable routes concurrently, then creates one RRF list per route.
async fn execute_routes<P: ModelPort>(
    context: &SearchContext<'_, P>,
    admitted: &AdmittedSearch,
) -> RouteResults {
    let query = Query {
        generation: &admitted.generation,
        scopes: &admitted.scopes,
        text: &admitted.understood.normalized,
        limit: DENSE_LIMIT,
        version: admitted.version.as_deref(),
        qdrant: context.qdrant,
    };
    let structured_request = match admitted.structured_error.as_deref() {
        Some(error) => Err(error),
        None => Ok(admitted.structured_request.as_ref()),
    };
    let configuration = admitted.configuration;
    let (dense, lexical, identifier, structured) = join_route_futures(
        traced_route(
            span::route_dense(),
            dense_outcome(
                configuration.dense_enabled,
                &query,
                context.embedder.as_ref(),
                &admitted.cutoffs,
            ),
        ),
        traced_route(
            span::route_lexical(),
            lexical_outcome(
                configuration.lexical_enabled,
                &query,
                admitted.cutoffs.routes,
            ),
        ),
        traced_route(
            span::route_identifier(),
            search_identifiers_enabled(
                configuration.identifier_enabled,
                &query,
                context.database.clone(),
                &admitted.understood,
                admitted.cutoffs.routes,
            ),
        ),
        traced_structured(
            admitted.understood.kind == QueryKind::Global,
            configuration.structured_enabled,
            structured_outcome(
                &query,
                context.database.clone(),
                structured_request,
                admitted.cutoffs.routes,
            ),
        ),
    )
    .await;
    let mut lists = vec![
        route_list(Route::Dense, &dense),
        route_list(Route::Lexical, &lexical),
        route_list(Route::Identifier, &identifier),
    ];
    if let Some(structured) = &structured {
        lists.push(route_list(Route::Structured, &structured.route));
    }
    let observations = route_observations(&dense, &lexical, &identifier, structured.as_ref());
    let fused = traced_fuse(&lists, configuration);
    let expected_revisions = revisions_for_fused(
        &fused,
        [&dense, &lexical, &identifier]
            .into_iter()
            .chain(structured.iter().map(|outcome| &outcome.route)),
    );
    let mut known_gaps = Vec::new();
    if !admitted.version_documented
        && let Some(version) = admitted.version.as_deref()
    {
        known_gaps.push(format!(
            "requested version {version:?} has no documents in the pinned generation"
        ));
    }
    let (routes, known_gaps, inventory) = route_metadata(
        &dense,
        &lexical,
        &identifier,
        structured.as_ref(),
        known_gaps,
    );
    RouteResults {
        fused,
        expected_revisions,
        routes,
        inventory,
        known_gaps,
        observations,
    }
}

/// Records each route's status, its degradation gaps and the structured inventory.
fn route_metadata(
    dense: &RouteOutcome,
    lexical: &RouteOutcome,
    identifier: &RouteOutcome,
    structured: Option<&StructuredOutcome>,
    mut known_gaps: Vec<String>,
) -> (
    BTreeMap<String, RouteStatus>,
    Vec<String>,
    Option<Inventory>,
) {
    let mut routes = BTreeMap::new();
    for (route, outcome) in [
        (Route::Dense, dense),
        (Route::Lexical, lexical),
        (Route::Identifier, identifier),
    ] {
        add_route(&mut routes, &mut known_gaps, route, &outcome.status);
    }
    let inventory = structured.and_then(|outcome| outcome.inventory.clone());
    if let Some(outcome) = structured {
        add_route(
            &mut routes,
            &mut known_gaps,
            Route::Structured,
            &outcome.route.status,
        );
    }
    (routes, known_gaps, inventory)
}

/// Captures each route's candidate order before fusion.
fn route_observations(
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
fn observed_chunk_ids(outcome: &RouteOutcome) -> Vec<String> {
    outcome
        .hits
        .iter()
        .map(|hit| hit.chunk_id.clone())
        .collect()
}

/// Fuses the routes' `lists` into one pool, as the fusion stage.
fn traced_fuse(
    lists: &[RouteList],
    configuration: super::request::SearchConfiguration,
) -> Vec<Fused> {
    let stage = span::fuse();
    let fused = stage.in_scope(|| {
        super::fusion::fuse_weighted(lists, FUSION_POOL, configuration.rrf_k, |route| {
            configuration.weight(route)
        })
    });
    stage.count(Count::Candidates, fused.len());
    stage.finish(Outcome::Ok);
    fused
}

/// Loads exact candidates, reranks until the setup cutoff, which leaves
/// evidence assembly its time, or degrades safely, and rechecks permissions.
async fn finish_search<P: ModelPort>(
    context: &SearchContext<'_, P>,
    request: &SearchRequest<'_>,
    admitted: AdmittedSearch,
    mut routes: RouteResults,
) -> Result<EvidenceInput, SearchError> {
    let candidates = if routes.fused.is_empty() {
        Vec::new()
    } else {
        ensure_permissions(
            context.database.clone(),
            context.principal,
            admitted.scopes.as_ref(),
            admitted.cutoffs.work,
        )
        .await?;
        match candidates::load(
            context.database.clone(),
            candidates::Request {
                generation: admitted.generation.clone(),
                scopes: admitted.scopes.as_ref().clone(),
                version: admitted.version.clone(),
                fused: mem::take(&mut routes.fused),
                expected_revisions: mem::take(&mut routes.expected_revisions),
                deadline: admitted.cutoffs.work,
            },
        )
        .await
        {
            Ok(candidates) => candidates,
            Err(CandidateFailure::TimedOut) => {
                routes
                    .known_gaps
                    .push("candidate text loading timed out".to_owned());
                routes.routes.insert(
                    "rerank".to_owned(),
                    RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned()),
                );
                ensure_permissions(
                    context.database.clone(),
                    context.principal,
                    admitted.scopes.as_ref(),
                    admitted.cutoffs.expires,
                )
                .await?;
                return Ok(evidence_input(request, admitted, routes, Vec::new()));
            }
            Err(CandidateFailure::EvidenceLoad) => {
                return Err(SearchError::EvidenceLoad {
                    reason:
                        "a fused candidate is unavailable, corrupt, or outside the pinned scope"
                            .to_owned(),
                });
            }
            Err(CandidateFailure::Kernel(error)) => return Err(SearchError::Kernel(error)),
            Err(CandidateFailure::WorkerFailed) => return Err(SearchError::WorkerFailed),
        }
    };
    let stage = span::rerank();
    let (ranked, rerank_status) = stage
        .instrument(rerank_candidates(
            &admitted.understood,
            candidates,
            context.reranker.as_ref(),
            admitted
                .configuration
                .rerank_enabled
                .then_some(admitted.configuration.rerank_depth),
            admitted.cutoffs.setup,
        ))
        .await;
    stage.count(Count::Candidates, ranked.len());
    stage.finish(route_outcome(&rerank_status));
    routes.observations.reranked_chunk_ids = ranked
        .iter()
        .map(|candidate| candidate.candidate.fused.chunk_id.clone())
        .collect();
    add_named_route(
        &mut routes.routes,
        &mut routes.known_gaps,
        "rerank",
        &rerank_status,
    );
    ensure_permissions(
        context.database.clone(),
        context.principal,
        admitted.scopes.as_ref(),
        admitted.cutoffs.expires,
    )
    .await?;
    Ok(evidence_input(request, admitted, routes, ranked))
}

/// Assembles the transport-neutral handoff without expanding source evidence.
fn evidence_input(
    request: &SearchRequest<'_>,
    admitted: AdmittedSearch,
    routes: RouteResults,
    ranked: Vec<Ranked>,
) -> EvidenceInput {
    EvidenceInput {
        generation: admitted.generation,
        query: request.text.to_owned(),
        understood: admitted.understood,
        version: admitted.version,
        principal: admitted.principal,
        scopes: admitted.scopes,
        ranked,
        routes: routes.routes,
        inventory: routes.inventory,
        budget: request.budget,
        deadline: admitted.cutoffs.expires,
        known_gaps: routes.known_gaps,
        observations: routes.observations,
    }
}

/// Retains revision identities for every fused candidate to validate kernel ownership.
fn revisions_for_fused<'a>(
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

/// Reranks loaded candidates or returns the safe fused-order fallback.
pub(super) async fn rerank_candidates<P: ModelPort>(
    understood: &Understood,
    candidates: Vec<Candidate>,
    reranker: Option<&Reranker<'_, P>>,
    depth: Option<NonZeroUsize>,
    deadline: Instant,
) -> (Vec<Ranked>, RouteStatus) {
    if candidates.is_empty() {
        return (
            Vec::new(),
            RouteStatus::Unavailable(NO_FUSED_CANDIDATES.to_owned()),
        );
    }
    let Some(depth) = depth else {
        return (
            fused_order(candidates),
            RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned()),
        );
    };
    let Some(reranker) = reranker else {
        return (
            fused_order(candidates),
            RouteStatus::Unavailable("no reranker configured".to_owned()),
        );
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return (
            fused_order(candidates),
            RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned()),
        );
    }
    let result = rerank(
        &understood.normalized,
        candidates,
        reranker,
        depth,
        remaining,
    )
    .await;
    (result.ranked, result.status)
}

/// Keeps the fused order when no reranker can run.
fn fused_order(candidates: Vec<Candidate>) -> Vec<Ranked> {
    candidates
        .into_iter()
        .map(|candidate| Ranked {
            candidate,
            score: None,
        })
        .collect()
}
