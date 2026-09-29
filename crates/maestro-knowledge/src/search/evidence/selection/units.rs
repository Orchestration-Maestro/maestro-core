//! Selection units: conflict-atomic groups, then ordinary candidates, in
//! original candidate order.

use super::super::types::EvidenceError;
use super::types::SelectionCandidate;
use std::collections::BTreeSet;

/// A conflict-atomic group or one ordinary candidate.
pub(super) struct SelectionUnit {
    /// Source candidate indexes grouped into this selection unit.
    pub(super) candidates: Vec<usize>,
    /// Whether all members must be selected atomically.
    pub(super) conflict: bool,
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

/// Converts helper diagnostics to the stable, text-free integrity boundary.
fn integrity(reason: &str) -> EvidenceError {
    EvidenceError::Integrity(reason.to_owned())
}
