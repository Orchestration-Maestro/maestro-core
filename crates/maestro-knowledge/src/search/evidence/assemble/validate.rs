//! Validates the bounded T029c handoff before any blocking work starts.

use super::super::super::{fusion::Route, request::EvidenceInput};
use super::super::types::EvidenceError;
use maestro_kernel::{evidence::RouteStatus, retrieval::normalize_whitespace};
use std::collections::BTreeSet;

/// Converts an invalid handoff field into a non-sensitive public error.
fn invalid(reason: &str) -> EvidenceError {
    EvidenceError::InvalidRequest(reason.to_owned())
}

/// Checks handoff bounds and verifies that ranking metadata is internally valid.
pub(super) fn validate_input(input: &EvidenceInput) -> Result<(), EvidenceError> {
    if input.understood.normalized.trim().is_empty()
        || input.principal.trim().is_empty()
        || input.query.len() > 8192
        || input
            .version
            .as_ref()
            .is_some_and(|version| version.is_empty() || version.len() > 8192)
        || normalize_whitespace(&input.query) != input.understood.normalized
    {
        return Err(invalid("evidence handoff text or identity is invalid"));
    }
    if !(1..=50).contains(&input.budget.k)
        || !(1..=12_000).contains(&input.budget.max_tokens)
        || !(1..=10_000).contains(&input.budget.deadline_ms)
        || input.ranked.len() > 120
    {
        return Err(invalid(
            "evidence handoff exceeds its bounded request limits",
        ));
    }
    let identifiers: BTreeSet<_> = input
        .understood
        .identifiers
        .iter()
        .map(|identifier| identifier.text.as_str())
        .collect();
    if identifiers.len() > 64
        || identifiers
            .iter()
            .any(|identifier| identifier.trim().is_empty())
    {
        return Err(invalid(
            "evidence handoff has too many or blank identifiers",
        ));
    }

    let rerank = input
        .routes
        .get("rerank")
        .ok_or_else(|| invalid("evidence handoff has no rerank status"))?;
    if input.routes.values().any(
        |status| matches!(status, RouteStatus::Unavailable(reason) if reason.trim().is_empty()),
    ) {
        return Err(invalid("evidence handoff has a blank unavailable reason"));
    }

    let mut chunk_ids = BTreeSet::new();
    for ranked in &input.ranked {
        let fused = &ranked.candidate.fused;
        if fused.chunk_id.trim().is_empty()
            || !chunk_ids.insert(fused.chunk_id.as_str())
            || !fused.score.is_finite()
            || ranked.score.is_some_and(|score| !score.is_finite())
            || fused.ranks.is_empty()
        {
            return Err(invalid("evidence handoff candidate metadata is invalid"));
        }
        if matches!(rerank, RouteStatus::Unavailable(_)) && ranked.score.is_some() {
            return Err(invalid(
                "unavailable reranking cannot supply candidate scores",
            ));
        }
        for route in fused.ranks.keys() {
            let status = input
                .routes
                .get(route.name())
                .ok_or_else(|| invalid("candidate route status is missing"))?;
            if matches!(status, RouteStatus::Unavailable(_)) && *route != Route::Identifier {
                return Err(invalid("an unavailable route supplied a candidate"));
            }
        }
    }
    Ok(())
}
