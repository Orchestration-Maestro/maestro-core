//! Admission, parallel route execution, fusion and the bounded T032 handoff.

use super::{
    admission::{AdmittedSearch, admit_request, ensure_permissions},
    candidates::{self, Failure as CandidateFailure},
    fusion::{Fused, Route, fuse},
    query::Query,
    request::{EvidenceInput, SearchContext, SearchError, SearchRequest},
    rerank::{Candidate, Ranked, Reranker, rerank},
    route_execution::{
        add_named_route, add_route, dense_outcome, join_route_futures, lexical_outcome, route_list,
        structured_outcome,
    },
    routes::{identifier::search_identifiers, outcome::RouteOutcome},
};
use crate::query::{QueryKind, Understood};
use maestro_kernel::{
    evidence::{Inventory, RouteStatus},
    gateway::ModelPort,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    mem,
    num::NonZeroUsize,
};
use tokio::time::Instant;

/// The dense and lexical routes' bounded candidate pool.
const DENSE_LIMIT: usize = 100;
/// The single reciprocal-rank fusion pool limit.
const FUSION_POOL: usize = 120;
/// Retrieves a pinned, scoped and deadline-bounded evidence handoff for T032.
///
/// # Errors
///
/// Returns [`SearchError`] when admission, permissions, or candidate integrity
/// cannot be established within the request deadline.
pub async fn search<P: ModelPort>(
    context: &SearchContext<'_, P>,
    request: &SearchRequest<'_>,
) -> Result<EvidenceInput, SearchError> {
    let admitted = admit_request(context, request).await?;
    let routes = execute_routes(context, &admitted).await;
    finish_search(context, request, admitted, routes).await
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
    let (dense, lexical, identifier, structured) = join_route_futures(
        dense_outcome(&query, context.embedder.as_ref(), admitted.cutoffs.routes),
        lexical_outcome(&query, admitted.cutoffs.routes),
        search_identifiers(
            &query,
            context.database.clone(),
            &admitted.understood,
            admitted.cutoffs.routes,
        ),
        structured_outcome(
            &query,
            context.database.clone(),
            structured_request,
            admitted.understood.kind == QueryKind::Global,
            admitted.cutoffs.routes,
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
    let fused = fuse(&lists, FUSION_POOL);
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
    let mut routes = BTreeMap::new();
    add_route(&mut routes, &mut known_gaps, Route::Dense, &dense.status);
    add_route(
        &mut routes,
        &mut known_gaps,
        Route::Lexical,
        &lexical.status,
    );
    add_route(
        &mut routes,
        &mut known_gaps,
        Route::Identifier,
        &identifier.status,
    );
    let inventory = structured
        .as_ref()
        .and_then(|outcome| outcome.inventory.clone());
    if let Some(structured) = &structured {
        add_route(
            &mut routes,
            &mut known_gaps,
            Route::Structured,
            &structured.route.status,
        );
    }
    RouteResults {
        fused,
        expected_revisions,
        routes,
        inventory,
        known_gaps,
    }
}

/// Loads exact candidates, reranks or degrades safely, and rechecks permissions.
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
                    RouteStatus::Unavailable("candidate text loading timed out".to_owned()),
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
    let (ranked, rerank_status) = rerank_candidates(
        &admitted.understood,
        candidates,
        context.reranker.as_ref(),
        request.rerank_depth,
        admitted.cutoffs.work,
    )
    .await;
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
    depth: NonZeroUsize,
    deadline: Instant,
) -> (Vec<Ranked>, RouteStatus) {
    if candidates.is_empty() {
        return (
            Vec::new(),
            RouteStatus::Unavailable("no fused candidates".to_owned()),
        );
    }
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
            RouteStatus::Unavailable("reranking deadline elapsed".to_owned()),
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
