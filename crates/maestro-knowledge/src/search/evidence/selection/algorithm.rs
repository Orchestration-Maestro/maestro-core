//! MMR ordering, atomic conflict units and budget-driven source windows.

use super::super::super::assembly_settings::ExpansionMode;
use super::super::{budget::count_passages, signals::OmissionStatus, types::EvidenceError};
use super::{
    mmr::{SelectionUnit, best_unit, selection_units, update_max_similarity},
    relevant::{relevant_span, table_prefixes},
    render::{RenderedTrial, include_span, render_trial},
    types::{SelectionBudget, SelectionCandidate, SelectionResult, check, integrity},
};
use maestro_kernel::{
    evidence::{Passage, Span},
    retrieval::ReadControl,
};
use std::collections::{BTreeMap, BTreeSet};

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
    let mut accepted = Vec::new();

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

        if budget.expansion == ExpansionMode::FullSection {
            let full = candidate_spans(candidates, &unit.candidates, |candidate| {
                Ok(candidate.expansion.extent)
            })?;
            let mut trial_spans = selected.spans.clone();
            trial_spans.extend(full);
            if try_spans(candidates, &mut selected, trial_spans, budget)? {
                continue;
            }
        }

        if budget.expansion == ExpansionMode::RelevantBlocks {
            let mut trial_spans = selected.spans.clone();
            let relevant = candidate_spans(candidates, &unit.candidates, relevant_span)?;
            let prefixes = table_prefixes(candidates, &relevant);
            trial_spans.extend(relevant);
            if try_spans(candidates, &mut selected, trial_spans, budget)? {
                accepted.extend_from_slice(&unit.candidates);
            } else {
                // Each relevant window lies within its mandatory window,
                // which cannot fit either.
                omissions.table_prefix_omissions += prefixes;
                omit(&mut omissions, &unit);
            }
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
            omit(&mut omissions, &unit);
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

    for index in accepted {
        add_optional_siblings(candidates, index, &mut selected, budget)?;
    }
    check(budget.control)?;
    Ok(SelectionResult {
        passages: selected.passages,
        selected_candidates: selected.candidates,
        omissions,
    })
}

/// Accepts a complete trial only when both passage and representation limits fit.
fn try_spans(
    candidates: &[SelectionCandidate<'_>],
    selected: &mut SelectionState,
    spans: BTreeMap<usize, Span>,
    budget: &SelectionBudget<'_>,
) -> Result<bool, EvidenceError> {
    let (fits, rendered) = fits_trial(candidates, &spans, budget)?;
    if fits {
        accept_trial(candidates, selected, spans, rendered, budget.control)?;
    }
    Ok(fits)
}

/// Records that `unit` did not fit the evidence or passage budget.
fn omit(omissions: &mut OmissionStatus, unit: &SelectionUnit) {
    omissions.evidence = true;
    omissions.conflict |= unit.conflict;
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
    let tokens = count_passages(
        &rendered.passages,
        budget.counter,
        budget.counter_info,
        budget.max_tokens,
    )
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
    for sibling in plan.neighbors() {
        let expanded = include_span(current, sibling)
            .map_err(|_| integrity("candidate optional sibling is invalid"))?;
        let mut trial_spans = selected.spans.clone();
        trial_spans.insert(candidate_index, expanded);
        let (fits, rendered) = fits_trial(candidates, &trial_spans, budget)?;
        if fits {
            accept_trial(candidates, selected, trial_spans, rendered, budget.control)?;
            current = expanded;
        }
    }
    Ok(())
}
