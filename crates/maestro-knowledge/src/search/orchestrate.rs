//! The search entry point: admission, then the whole question's ranking,
//! and its parts' when it splits, as the bounded T032 handoff.

use super::{
    admission::admit_request,
    part_search, ranked,
    request::{EvidenceInput, SearchContext, SearchError, SearchRequest},
};
use maestro_kernel::{
    gateway::ModelPort,
    telemetry::{
        span,
        stage::{Count, Outcome},
    },
};

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
            match part_search::parts(context, &admitted) {
                Some(split) => {
                    Box::pin(part_search::search(context, request, admitted, split)).await
                }
                None => Box::pin(ranked::search(context, request, admitted)).await,
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
