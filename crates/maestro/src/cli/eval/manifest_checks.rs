//! Validation for search behavior configured by ladder rungs.

use crate::failure::Failure;
use maestro_kernel::artifact::Digest;
use maestro_knowledge::search::{IntentExpansion, IntentTrigger, SearchConfiguration};

/// Validates limits shared by search routes, fusion and reranking.
pub(super) fn check_pipeline_limits(
    routes_limit: usize,
    identifier_limit: usize,
    fusion_pool: usize,
    rerank_depth: Option<usize>,
) -> Result<(), Failure> {
    if !(1..=SearchConfiguration::MAX_FUSION_POOL).contains(&routes_limit) {
        return Err(Failure::refused(
            "a rung's routes_limit must be between 1 and 120",
        ));
    }
    if !(1..=SearchConfiguration::MAX_FUSION_POOL).contains(&identifier_limit) {
        return Err(Failure::refused(
            "a rung's identifier_limit must be between 1 and 120",
        ));
    }
    if !(1..=SearchConfiguration::MAX_FUSION_POOL).contains(&fusion_pool) {
        return Err(Failure::refused(
            "a rung's fusion_pool must be between 1 and 120",
        ));
    }
    if rerank_depth.is_some_and(|depth| depth > fusion_pool) {
        return Err(Failure::refused(
            "a rung's rerank depth cannot exceed its fusion pool",
        ));
    }
    Ok(())
}

/// Checks the opt-in expansion card and bounded model deadline before a run.
pub(super) fn check_intent(
    expansion: IntentExpansion,
    trigger: IntentTrigger,
    deadline_ms: u32,
    rerank_additions: usize,
    card: Option<&str>,
) -> Result<(), Failure> {
    if !trigger.is_valid() {
        return Err(Failure::refused(
            "intent confidence threshold must be finite",
        ));
    }
    if !(1..=5000).contains(&deadline_ms) {
        return Err(Failure::refused(
            "intent deadline must be between 1 and 5000 milliseconds",
        ));
    }
    if rerank_additions > 120 {
        return Err(Failure::refused(
            "intent rerank additions must be at most 120",
        ));
    }
    if expansion == IntentExpansion::Hyde && card.is_none() {
        return Err(Failure::refused("hyde requires an explicit intent card"));
    }
    if let Some(card) = card {
        Digest::parse(card)
            .map_err(|_| Failure::refused("intent card must be a SHA-256 digest"))?;
    }
    Ok(())
}
