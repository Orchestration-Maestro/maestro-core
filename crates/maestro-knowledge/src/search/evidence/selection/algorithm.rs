//! MMR ordering, atomic conflict units and budget-driven source windows.

use super::super::{
    budget::count_passages,
    features::{diversity_similarity, mmr_score},
    signals::OmissionStatus,
    types::EvidenceError,
};
use super::{
    render::{RenderedTrial, include_span, render_trial},
    types::{SelectionBudget, SelectionCandidate, SelectionResult},
};
use maestro_kernel::{
    evidence::{Passage, Span},
    retrieval::ReadControl,
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    sync::atomic::Ordering as AtomicOrdering,
    time::Instant,
};

/// Selects complete candidate units by MMR and serialized budget, shrinking only to safe windows.
pub(crate) fn select(
    candidates: &[SelectionCandidate<'_>],
    conflict_units: &[BTreeSet<usize>],
    budget: &SelectionBudget<'_>,
) -> Result<SelectionResult, EvidenceError> {
    check(budget.control)?;
    let mut pending = selection_units(candidates, conflict_units)?;
    let mut selected = SelectionState {
        max_similarity: vec![0.0; candidates.len()],
        ..SelectionState::default()
    };
    let mut omissions = OmissionStatus::default();

    while !pending.is_empty() {
        check(budget.control)?;
        let Some(best_position) = best_unit(
            &pending,
            candidates,
            &selected.candidates,
            &selected.max_similarity,
            budget.control,
        )?
        else {
            break;
        };
        let unit = pending.remove(best_position);
        if unit
            .candidates
            .iter()
            .all(|index| selected.candidates.contains(index))
        {
            continue;
        }
        if unit.conflict
            && unit
                .candidates
                .iter()
                .any(|index| selected.candidates.contains(index))
        {
            return Err(integrity("a conflict unit was only partly covered"));
        }

        let full = candidate_spans(candidates, &unit.candidates, |candidate| {
            Ok(candidate.expansion.extent)
        })?;
        let mut trial_spans = selected.spans.clone();
        trial_spans.extend(full);
        let (fits, rendered) = fits_trial(candidates, &trial_spans, budget)?;
        if fits {
            accept_trial(
                candidates,
                &mut selected,
                trial_spans,
                rendered,
                budget.control,
            )?;
            continue;
        }

        let mandatory = candidate_spans(candidates, &unit.candidates, |candidate| {
            candidate
                .expansion
                .window_plan(candidate.required_span)
                .map(|plan| plan.mandatory)
                .map_err(|_| integrity("candidate has no safe mandatory source window"))
        })?;
        let mut trial_spans = selected.spans.clone();
        trial_spans.extend(mandatory);
        let (fits, rendered) = fits_trial(candidates, &trial_spans, budget)?;
        if !fits {
            omissions.evidence = true;
            omissions.conflict |= unit.conflict;
            continue;
        }

        accept_trial(
            candidates,
            &mut selected,
            trial_spans,
            rendered,
            budget.control,
        )?;
        for index in &unit.candidates {
            add_optional_siblings(candidates, *index, &mut selected, budget)?;
        }
    }

    check(budget.control)?;
    Ok(SelectionResult {
        passages: selected.passages,
        selected_candidates: selected.candidates,
        omissions,
    })
}

/// A conflict-atomic group or one ordinary candidate.
struct SelectionUnit {
    /// Source candidate indexes grouped into this selection unit.
    candidates: Vec<usize>,
    /// Whether all members must be selected atomically.
    conflict: bool,
}

/// Current atomic selection and its rendered source passages.
#[derive(Default)]
struct SelectionState {
    /// Selected source windows by candidate index.
    spans: BTreeMap<usize, Span>,
    /// Current passages rendered from those windows.
    passages: Vec<Passage>,
    /// Candidate indexes whose required spans are represented.
    candidates: BTreeSet<usize>,
    /// Maximum similarity to any newly selected candidate, updated incrementally.
    max_similarity: Vec<f64>,
}

impl SelectionState {
    /// Replaces the current selection with one fully measured trial.
    fn accept(&mut self, spans: BTreeMap<usize, Span>, rendered: RenderedTrial) {
        self.spans = spans;
        self.passages = rendered.passages;
        self.candidates = rendered.covered_candidates;
    }
}

/// Accepts a trial and updates pairwise maxima only for newly covered candidates.
fn accept_trial(
    candidates: &[SelectionCandidate<'_>],
    selected: &mut SelectionState,
    spans: BTreeMap<usize, Span>,
    rendered: RenderedTrial,
    control: &ReadControl,
) -> Result<(), EvidenceError> {
    let newly_covered = rendered
        .covered_candidates
        .difference(&selected.candidates)
        .copied()
        .collect::<Vec<_>>();
    selected.accept(spans, rendered);
    update_max_similarity(
        candidates,
        &newly_covered,
        &selected.candidates,
        &mut selected.max_similarity,
        control,
    )
}

/// Incorporates each new selection into every candidate's cached maximum similarity.
fn update_max_similarity(
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
            if candidate_index == *selected_index || selected_candidates.contains(&candidate_index)
            {
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
fn selection_units(
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
fn best_unit(
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

/// Builds candidate-indexed source spans for one atomic trial.
fn candidate_spans<'a>(
    candidates: &[SelectionCandidate<'a>],
    indices: &[usize],
    span: impl Fn(&SelectionCandidate<'a>) -> Result<Span, EvidenceError>,
) -> Result<BTreeMap<usize, Span>, EvidenceError> {
    indices
        .iter()
        .map(|index| {
            let candidate = candidates
                .get(*index)
                .ok_or_else(|| integrity("selection candidate index is invalid"))?;
            Ok((*index, span(candidate)?))
        })
        .collect()
}

/// Renders one trial and measures its full compact passage JSON.
fn fits_trial(
    candidates: &[SelectionCandidate<'_>],
    spans: &BTreeMap<usize, Span>,
    budget: &SelectionBudget<'_>,
) -> Result<(bool, RenderedTrial), EvidenceError> {
    check(budget.control)?;
    let rendered = render_trial(candidates, spans)
        .map_err(|_| integrity("candidate trial could not be rendered"))?;
    check(budget.control)?;
    if rendered.passages.len() > budget.max_passages {
        return Ok((false, rendered));
    }
    let tokens = count_passages(&rendered.passages, budget.counter, budget.counter_info)
        .map_err(EvidenceError::from)?;
    check(budget.control)?;
    Ok((tokens <= budget.max_tokens, rendered))
}

/// Adds at most two source siblings on either side while each whole trial fits.
fn add_optional_siblings(
    candidates: &[SelectionCandidate<'_>],
    candidate_index: usize,
    selected: &mut SelectionState,
    budget: &SelectionBudget<'_>,
) -> Result<(), EvidenceError> {
    let candidate = candidates
        .get(candidate_index)
        .ok_or_else(|| integrity("selection candidate index is invalid"))?;
    let plan = candidate
        .expansion
        .window_plan(candidate.required_span)
        .map_err(|_| integrity("candidate has no safe optional source window"))?;
    let mut current = selected
        .spans
        .get(&candidate_index)
        .copied()
        .ok_or_else(|| integrity("selected mandatory window disappeared"))?;
    let neighbors = [
        plan.before.first().copied().map(|span| (true, span)),
        plan.after.first().copied().map(|span| (false, span)),
        plan.before.get(1).copied().map(|span| (true, span)),
        plan.after.get(1).copied().map(|span| (false, span)),
    ];
    let mut before_closed = false;
    let mut after_closed = false;
    for (before, sibling) in neighbors.into_iter().flatten() {
        if (before && before_closed) || (!before && after_closed) {
            continue;
        }
        let expanded = include_span(current, sibling)
            .map_err(|_| integrity("candidate optional sibling is invalid"))?;
        let mut trial_spans = selected.spans.clone();
        trial_spans.insert(candidate_index, expanded);
        let (fits, rendered) = fits_trial(candidates, &trial_spans, budget)?;
        if fits {
            accept_trial(candidates, selected, trial_spans, rendered, budget.control)?;
            current = expanded;
        } else if before {
            before_closed = true;
        } else {
            after_closed = true;
        }
    }
    Ok(())
}

/// Stops before or after trial work when cancellation or expiry fires.
fn check(control: &ReadControl) -> Result<(), EvidenceError> {
    if control.cancelled.load(AtomicOrdering::Relaxed) || Instant::now() >= control.deadline {
        Err(EvidenceError::TimedOut)
    } else {
        Ok(())
    }
}

/// Converts helper diagnostics to the stable, text-free integrity boundary.
fn integrity(reason: &str) -> EvidenceError {
    EvidenceError::Integrity(reason.to_owned())
}
