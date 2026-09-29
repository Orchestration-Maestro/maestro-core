//! Admission, parallel route execution, fusion and the bounded T032 handoff.

use super::{
    admission::{AdmittedSearch, admit_request, ensure_permissions},
    candidates::Failure as CandidateFailure,
    deadline::DEADLINE_EXCEEDED,
    fusion::Route,
    intent::{IntentExpansion, IntentTrigger},
    intent_routes, rank_stage,
    request::{EvidenceInput, SearchContext, SearchError, SearchRequest},
    rerank::{Ranked, top_rerank_score},
    route_execution::{add_named_route, prepare_reranker},
    route_search::{self, Originals, RouteResults},
};
use maestro_kernel::{
    evidence::RouteStatus,
    gateway::ModelPort,
    telemetry::{
        span,
        stage::{Count, Outcome},
    },
};
use std::mem;

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
            // Fusion runs as soon as the routes end, while the reranker may
            // still be readying.
            let ((originals, routes), ()) = tokio::join!(
                Box::pin(async {
                    let (originals, intent) = route_search::execute(context, &admitted).await;
                    let routes = route_search::results(&admitted, &originals, intent);
                    (originals, routes)
                }),
                prepare_reranker(
                    context.reranker.as_ref(),
                    admitted.configuration.rerank_enabled,
                    &admitted.cutoffs,
                ),
            );
            if admitted.configuration.intent_expansion == IntentExpansion::Hyde
                && matches!(
                    admitted.configuration.intent_trigger,
                    IntentTrigger::LowConfidence { .. }
                )
            {
                conditional_search(context, request, admitted, originals, routes).await
            } else {
                finish_search(context, request, admitted, routes).await
            }
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

/// Reuses the original votes only when the first ranking needs expansion,
/// judged by its best rerank score, whatever rank policies put first. An
/// expansion that adds nothing, or a second pass that ranks worse, keeps
/// the first ranking with the intent statuses attached.
async fn conditional_search<P: ModelPort>(
    context: &SearchContext<'_, P>,
    request: &SearchRequest<'_>,
    admitted: AdmittedSearch,
    originals: Originals,
    routes: RouteResults,
) -> Result<EvidenceInput, SearchError> {
    let mut first = finish_search(context, request, admitted.clone(), routes).await?;
    if !admitted
        .configuration
        .intent_trigger
        .should_expand(top_rerank_score(&first.ranked))
    {
        first.routes.insert(
            "intent_expansion".to_owned(),
            RouteStatus::Unavailable("intent_not_triggered".to_owned()),
        );
        return Ok(first);
    }
    let intent = intent_routes::execute(context, &admitted).await;
    if intent
        .outcomes
        .iter()
        .all(|(_, outcome)| outcome.hits.is_empty())
    {
        if let Some(status) = intent.status {
            first.routes.insert("intent_expansion".to_owned(), status);
        }
        for (route, outcome) in intent.outcomes {
            first.routes.insert(route.name().to_owned(), outcome.status);
            first.observations.route_ranks.insert(route, Vec::new());
        }
        ensure_permissions(
            context.database.clone(),
            context.principal,
            &admitted.scopes,
            admitted.cutoffs.expires,
        )
        .await?;
        return Ok(first);
    }
    let mut routes = route_search::results(&admitted, &originals, intent);
    // The reranker scores only what the first pass did not.
    routes.known_scores = first
        .ranked
        .iter()
        .filter_map(|item| {
            item.score
                .map(|score| (item.candidate.fused.chunk_id.clone(), score))
        })
        .collect();
    let second = finish_search(context, request, admitted, routes).await?;
    if ranks_as_well(&first, &second) {
        Ok(second)
    } else {
        Ok(keep_first(first, &second))
    }
}

/// Whether the `second` pass ranked at least as well as the `first`: it
/// reranked whenever the first did, and loaded candidates whenever the
/// first had some.
fn ranks_as_well(first: &EvidenceInput, second: &EvidenceInput) -> bool {
    let reranked = |input: &EvidenceInput| input.routes.get("rerank") == Some(&RouteStatus::Ok);
    (reranked(second) || !reranked(first)) && (!second.ranked.is_empty() || first.ranked.is_empty())
}

/// The `first` ranking with the `second` pass's intent routes, and the
/// reason the second pass was not kept.
fn keep_first(mut first: EvidenceInput, second: &EvidenceInput) -> EvidenceInput {
    for route in [Route::DenseIntent, Route::LexicalIntent] {
        if let Some(status) = second.routes.get(route.name()) {
            first.routes.insert(route.name().to_owned(), status.clone());
        }
        if let Some(ranks) = second.observations.route_ranks.get(&route) {
            first.observations.route_ranks.insert(route, ranks.clone());
        }
    }
    first.observations.intent_displaced = second.observations.intent_displaced;
    first.routes.insert(
        "intent_expansion".to_owned(),
        RouteStatus::Unavailable("intent_second_pass_unavailable".to_owned()),
    );
    first
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

/// Loads exact candidates, reranks until the setup cutoff, which leaves
/// evidence assembly its time, or degrades safely, and rechecks permissions.
async fn finish_search<P: ModelPort>(
    context: &SearchContext<'_, P>,
    request: &SearchRequest<'_>,
    admitted: AdmittedSearch,
    mut routes: RouteResults,
) -> Result<EvidenceInput, SearchError> {
    if !routes.fused.is_empty() {
        ensure_permissions(
            context.database.clone(),
            context.principal,
            admitted.scopes.as_ref(),
            admitted.cutoffs.work,
        )
        .await?;
    }
    let pool = rank_stage::Pool {
        fused: mem::take(&mut routes.fused),
        expected_revisions: mem::take(&mut routes.expected_revisions),
        rerank_extra: routes.rerank_extra,
        known_scores: mem::take(&mut routes.known_scores),
    };
    let ranking = match rank_stage::rank(
        context.database.clone(),
        context.reranker.as_ref(),
        &admitted,
        request.text,
        pool,
    )
    .await
    {
        Ok(ranking) => ranking,
        Err(CandidateFailure::TimedOut) => {
            candidate_timeout(&mut routes);
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
                reason: "a fused candidate is unavailable, corrupt, or outside the pinned scope"
                    .to_owned(),
            });
        }
        Err(CandidateFailure::Kernel(error)) => return Err(SearchError::Kernel(error)),
        Err(CandidateFailure::WorkerFailed) => return Err(SearchError::WorkerFailed),
    };
    routes.observations.candidate_source_load_micros = ranking.source_load_micros;
    routes.known_gaps.extend(ranking.context_gap());
    routes.observations.candidate_context_fallbacks = ranking.fallbacks;
    routes.observations.reranked_chunk_ids = ranking
        .ranked
        .iter()
        .map(|candidate| candidate.candidate.fused.chunk_id.clone())
        .collect();
    add_named_route(
        &mut routes.routes,
        &mut routes.known_gaps,
        "rerank",
        &ranking.status,
    );
    ensure_permissions(
        context.database.clone(),
        context.principal,
        admitted.scopes.as_ref(),
        admitted.cutoffs.expires,
    )
    .await?;
    Ok(evidence_input(request, admitted, routes, ranking.ranked))
}

/// Records candidate-loading deadline degradation before the permission recheck.
fn candidate_timeout(routes: &mut RouteResults) {
    routes
        .known_gaps
        .push("candidate text loading timed out".to_owned());
    routes.routes.insert(
        "rerank".to_owned(),
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned()),
    );
}

/// Assembles the transport-neutral handoff without expanding source evidence.
fn evidence_input(
    request: &SearchRequest<'_>,
    admitted: AdmittedSearch,
    routes: RouteResults,
    ranked: Vec<Ranked>,
) -> EvidenceInput {
    EvidenceInput {
        evidence: request.evidence,
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
