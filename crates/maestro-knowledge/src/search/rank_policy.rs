//! Optional position-only rerank policies; model and fusion scores remain untouched.

use super::rerank::Ranked;
use std::{collections::BTreeMap, num::NonZeroU32};

/// Fuses reranked and fused positions as rank fusion does,
/// `(1 - w) / (k + reranked + 1) + w / (k + fused + 1)`, with the search's
/// RRF constant `k`; equal blends keep the fused order.
#[expect(
    clippy::cast_precision_loss,
    reason = "search bounds the candidate pool to 120"
)]
pub(super) fn blend(
    ranked: &mut [Ranked],
    fused_ids: &[String],
    fused_weight: Option<f32>,
    rrf_k: NonZeroU32,
) {
    let Some(weight) = fused_weight else {
        return;
    };
    let weight = f64::from(weight);
    let offset = f64::from(rrf_k.get());
    let mut positions = ranked
        .iter()
        .enumerate()
        .map(|(position, item)| {
            let fused = fused_ids
                .iter()
                .position(|id| id == &item.candidate.fused.chunk_id)
                .unwrap_or(position);
            let score = (1.0 - weight) / (offset + position as f64 + 1.0)
                + weight / (offset + fused as f64 + 1.0);
            (item.clone(), score, fused)
        })
        .collect::<Vec<_>>();
    positions.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.2.cmp(&right.2)));
    for (target, (item, _, _)) in ranked.iter_mut().zip(positions) {
        *target = item;
    }
}

/// Moves each fused top-ten item up to at most `cap` places below its fused
/// position, in fused order, so every earlier bound still holds.
pub(super) fn cap_demotion(ranked: &mut [Ranked], fused_ids: &[String], cap: Option<u16>) {
    let Some(cap) = cap else {
        return;
    };
    for (fused, id) in fused_ids.iter().take(10).enumerate() {
        let bound = fused + usize::from(cap);
        if let Some(position) = ranked
            .iter()
            .position(|item| &item.candidate.fused.chunk_id == id)
            && let Some(window) = ranked.get_mut(bound.min(position)..=position)
        {
            window.rotate_right(1);
        }
    }
}

/// Multiplies each candidate's reciprocal final-rank score `1 / (position +
/// 1)` by its multiplier in `multipliers` (1 when absent) and reorders by
/// the result, so a penalized rank r moves to about r / multiplier; on equal
/// scores the less penalized candidate goes first, so any penalty demotes
/// strictly. No penalty leaves the order untouched.
#[expect(
    clippy::cast_precision_loss,
    reason = "search bounds the candidate pool to 120"
)]
pub(super) fn demote(ranked: &mut [Ranked], multipliers: &BTreeMap<String, f64>) {
    if multipliers.is_empty() {
        return;
    }
    let mut ordered = ranked
        .iter()
        .enumerate()
        .map(|(position, item)| {
            let multiplier = multipliers
                .get(&item.candidate.fused.chunk_id)
                .copied()
                .unwrap_or(1.0);
            (item.clone(), multiplier / (position + 1) as f64, multiplier)
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| right.1.total_cmp(&left.1).then(right.2.total_cmp(&left.2)));
    for (target, (item, _, _)) in ranked.iter_mut().zip(ordered) {
        *target = item;
    }
}
