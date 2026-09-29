//! Deterministic MMR ranking and conflict-atomic selection units.

use super::super::selection_candidate::SelectionCandidate;
use super::super::{
    features::{diversity_similarity, mmr_score},
    types::EvidenceError,
};
use super::types::{check, integrity};
use maestro_kernel::retrieval::ReadControl;
use std::{cmp::Ordering, collections::BTreeSet};

/// A conflict-atomic group or one ordinary candidate.
pub(super) struct SelectionUnit {
    /// Source candidate indexes grouped into this selection unit.
    pub(super) candidates: Vec<usize>,
    /// Whether all members must be selected atomically.
    pub(super) conflict: bool,
}

/// Incorporates each new selection into every candidate's cached maximum similarity.
pub(super) fn update_max_similarity(
    candidates: &[SelectionCandidate<'_>],
    newly_covered: &[usize],
    selected_candidates: &BTreeSet<usize>,
    max_similarity: &mut [f64],
    control: &ReadControl,
) -> Result<(), EvidenceError> {
    for selected_index in newly_covered {
        let newly_selected = candidates
            .get(*selected_index)
            .ok_or_else(|| integrity("selected candidate index is invalid"))?;
        for (candidate_index, candidate) in candidates.iter().enumerate() {
            if selected_candidates.contains(&candidate_index) {
                continue;
            }
            check(control)?;
            let similarity = diversity_similarity(&candidate.features, &newly_selected.features)
                .map_err(|_| integrity("candidate diversity features are invalid"))?;
            let maximum = max_similarity
                .get_mut(candidate_index)
                .ok_or_else(|| integrity("candidate similarity index is invalid"))?;
            *maximum = maximum.max(similarity);
        }
    }
    Ok(())
}

/// Merges overlapping conflict groups before adding ordinary singleton candidates.
pub(super) fn selection_units(
    candidates: &[SelectionCandidate<'_>],
    conflict_units: &[BTreeSet<usize>],
) -> Result<Vec<SelectionUnit>, EvidenceError> {
    // ponytail: O(n²) group merging at the 120-candidate cap; use disjoint sets if it rises.
    let mut groups = Vec::<BTreeSet<usize>>::new();
    for incoming in conflict_units {
        if incoming.is_empty() {
            return Err(integrity("conflict unit has no candidates"));
        }
        if incoming
            .iter()
            .any(|index| candidates.get(*index).is_none())
        {
            return Err(integrity("conflict unit names a missing candidate"));
        }
        let mut merged = incoming.clone();
        loop {
            let overlap = groups.iter().position(|group| !group.is_disjoint(&merged));
            let Some(overlap) = overlap else {
                break;
            };
            let group = groups.remove(overlap);
            merged.extend(group);
        }
        groups.push(merged);
    }
    let mut conflicted = BTreeSet::new();
    let mut units = Vec::new();
    for group in groups {
        conflicted.extend(group.iter().copied());
        units.push(SelectionUnit {
            candidates: ordered_candidates(candidates, group)?,
            conflict: true,
        });
    }
    for index in 0..candidates.len() {
        if !conflicted.contains(&index) {
            units.push(SelectionUnit {
                candidates: vec![index],
                conflict: false,
            });
        }
    }
    units.sort_by_key(|unit| {
        unit.candidates
            .iter()
            .filter_map(|index| {
                candidates
                    .get(*index)
                    .map(|candidate| (candidate.input_position, *index))
            })
            .min()
            .unwrap_or((usize::MAX, usize::MAX))
    });
    Ok(units)
}

/// Sorts a unit by original candidate order, then stable index.
fn ordered_candidates(
    candidates: &[SelectionCandidate<'_>],
    indices: BTreeSet<usize>,
) -> Result<Vec<usize>, EvidenceError> {
    let mut ordered = indices.into_iter().collect::<Vec<_>>();
    if ordered.iter().any(|index| candidates.get(*index).is_none()) {
        return Err(integrity("selection unit names a missing candidate"));
    }
    ordered.sort_by_key(|index| {
        candidates
            .get(*index)
            .map_or((usize::MAX, *index), |candidate| {
                (candidate.input_position, *index)
            })
    });
    Ok(ordered)
}

/// Returns the pending unit with the highest member MMR score.
pub(super) fn best_unit(
    units: &[SelectionUnit],
    candidates: &[SelectionCandidate<'_>],
    selected: &BTreeSet<usize>,
    max_similarity: &[f64],
    control: &ReadControl,
) -> Result<Option<usize>, EvidenceError> {
    // ponytail: pairwise similarity updates are O(n²) at the 120-candidate cap.
    let mut best: Option<(usize, f64, usize, usize)> = None;
    for (unit_position, unit) in units.iter().enumerate() {
        let mut unit_best: Option<(f64, usize, usize)> = None;
        for candidate_index in &unit.candidates {
            if selected.contains(candidate_index) {
                continue;
            }
            check(control)?;
            let candidate = candidates
                .get(*candidate_index)
                .ok_or_else(|| integrity("selection candidate index is invalid"))?;
            let similarity = *max_similarity
                .get(*candidate_index)
                .ok_or_else(|| integrity("candidate similarity index is invalid"))?;
            let score = mmr_score(candidate.input_position, similarity)
                .map_err(|_| integrity("candidate MMR rank is invalid"))?;
            let choice = (score, candidate.input_position, *candidate_index);
            if unit_best.is_none_or(|current| compare_choices(choice, current) == Ordering::Greater)
            {
                unit_best = Some(choice);
            }
        }
        if let Some((score, rank, candidate_index)) = unit_best {
            let choice = (unit_position, score, rank, candidate_index);
            if best.is_none_or(|current| compare_unit_choices(choice, current) == Ordering::Greater)
            {
                best = Some(choice);
            }
        }
    }
    Ok(best.map(|(position, _, _, _)| position))
}

/// Compares scores descending and deterministic input order ascending.
fn compare_choices(left: (f64, usize, usize), right: (f64, usize, usize)) -> Ordering {
    left.0
        .total_cmp(&right.0)
        .then_with(|| right.1.cmp(&left.1))
        .then_with(|| right.2.cmp(&left.2))
}

/// Adds the pending position as the final deterministic tie break.
fn compare_unit_choices(
    left: (usize, f64, usize, usize),
    right: (usize, f64, usize, usize),
) -> Ordering {
    left.1
        .total_cmp(&right.1)
        .then_with(|| right.2.cmp(&left.2))
        .then_with(|| right.3.cmp(&left.3))
        .then_with(|| right.0.cmp(&left.0))
}
