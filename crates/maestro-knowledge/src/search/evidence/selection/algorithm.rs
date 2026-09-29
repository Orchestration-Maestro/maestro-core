//! MMR ordering, atomic conflict units and budget-driven source windows.

use super::super::super::assembly_settings::{ExpansionMode, ParentChainOrder};
use super::super::delivery_graph::{ChoiceKind, DeliveryChoice};
use super::super::selection_candidate::SelectionCandidate;
use super::super::{signals::OmissionStatus, types::EvidenceError};
use super::{
    mmr::{SelectionUnit, best_unit, selection_units, update_max_similarity},
    relevant::{relevant_span, table_prefixes},
    render::{RenderedTrial, include_span, parent_supports, primary_contributions},
    trial::fits_trial,
    types::{SelectionBudget, SelectionResult, check, integrity},
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
    let mut parent_accepted = Vec::new();

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

        if budget.expansion == ExpansionMode::ParentChain {
            let choices = unit_choices(candidates, &unit, budget)?;
            let admitted = admit_parent(candidates, &mut selected, &unit, &choices, budget)?;
            if let Some(tier) = admitted {
                parent_accepted.push((unit, tier));
            } else {
                omissions.table_prefix_omissions += header_choices(&choices);
                omit(&mut omissions, &unit);
            }
            continue;
        }

        if select_legacy(candidates, &unit, &mut selected, budget, &mut omissions)? {
            accepted.extend_from_slice(&unit.candidates);
        }
    }

    for index in accepted {
        add_optional_siblings(candidates, index, &mut selected, budget)?;
    }
    for (unit, admitted_tier) in parent_accepted {
        let choices = unit_choices(candidates, &unit, budget)?;
        let tiers = choices.iter().map(Vec::len).max().unwrap_or(0);
        for tier in admitted_tier + 1..tiers {
            let trial = parent_trial(&selected.spans, &unit, &choices, tier);
            try_spans(candidates, &mut selected, trial, budget)?;
        }
    }
    check(budget.control)?;
    Ok(SelectionResult {
        primary_contributions: if budget.expansion == ExpansionMode::ParentChain {
            primary_contributions(&selected.passages, candidates, &selected.spans)
        } else {
            BTreeMap::new()
        },
        parent_supports: if budget.expansion == ExpansionMode::ParentChain {
            parent_supports(&selected.passages, candidates, &selected.spans)
        } else {
            BTreeMap::new()
        },
        passages: selected.passages,
        selected_candidates: selected.candidates,
        omissions,
    })
}

/// Accepts a complete trial only when both passage and representation limits fit.
fn try_spans(
    candidates: &[SelectionCandidate<'_>],
    selected: &mut SelectionState,
    spans: BTreeMap<usize, DeliveryChoice>,
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
    spans: BTreeMap<usize, DeliveryChoice>,
    /// Current passages rendered from those windows.
    passages: Vec<Passage>,
    /// Candidate indexes whose required spans are represented.
    candidates: BTreeSet<usize>,
    /// Maximum similarity to any newly selected candidate, updated incrementally.
    max_similarity: Vec<f64>,
}

impl SelectionState {
    /// Replaces the current selection with one fully measured trial.
    fn accept(&mut self, spans: BTreeMap<usize, DeliveryChoice>, rendered: RenderedTrial) {
        self.spans = spans;
        self.passages = rendered.passages;
        self.candidates = rendered.covered_candidates;
    }
}

/// Accepts a trial and updates pairwise maxima only for newly covered candidates.
fn accept_trial(
    candidates: &[SelectionCandidate<'_>],
    selected: &mut SelectionState,
    spans: BTreeMap<usize, DeliveryChoice>,
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
) -> Result<BTreeMap<usize, DeliveryChoice>, EvidenceError> {
    indices
        .iter()
        .map(|index| {
            let candidate = candidates
                .get(*index)
                .ok_or_else(|| integrity("selection candidate index is invalid"))?;
            Ok((
                *index,
                DeliveryChoice::canonical(candidate, vec![span(candidate)?], ChoiceKind::Unit),
            ))
        })
        .collect()
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
        .and_then(|choice| choice.context.first())
        .copied()
        .ok_or_else(|| integrity("selected mandatory window disappeared"))?;
    for sibling in plan.neighbors() {
        let expanded = include_span(current, sibling)
            .map_err(|_| integrity("candidate optional sibling is invalid"))?;
        let mut trial_spans = selected.spans.clone();
        trial_spans.insert(
            candidate_index,
            DeliveryChoice::canonical(candidate, vec![expanded], ChoiceKind::Unit),
        );
        let (fits, rendered) = fits_trial(candidates, &trial_spans, budget)?;
        if fits {
            accept_trial(candidates, selected, trial_spans, rendered, budget.control)?;
            current = expanded;
        }
    }
    Ok(())
}

/// Gets complete choices for every member of one atomic selection unit.
fn unit_choices(
    candidates: &[SelectionCandidate<'_>],
    unit: &SelectionUnit,
    budget: &SelectionBudget<'_>,
) -> Result<Vec<Vec<DeliveryChoice>>, EvidenceError> {
    unit.candidates
        .iter()
        .map(|index| {
            check(budget.control)?;
            let candidate = candidates
                .get(*index)
                .ok_or_else(|| integrity("selection candidate index is invalid"))?;
            let choices = budget.graph.choices(candidate)?;
            if choices.is_empty() {
                return Err(integrity("candidate has no delivery choices"));
            }
            Ok(choices)
        })
        .collect()
}

/// Builds a lockstep tier, clamping members with shorter parent chains.
fn parent_trial(
    selected: &BTreeMap<usize, DeliveryChoice>,
    unit: &SelectionUnit,
    choices: &[Vec<DeliveryChoice>],
    tier: usize,
) -> BTreeMap<usize, DeliveryChoice> {
    let mut trial = selected.clone();
    for (index, chain) in unit.candidates.iter().zip(choices) {
        if let Some(choice) = chain.get(tier).or_else(|| chain.last()) {
            trial.insert(*index, choice.clone());
        }
    }
    trial
}

/// Counts candidates requiring table headers, once per omitted unit member.
fn header_choices(choices: &[Vec<DeliveryChoice>]) -> usize {
    choices
        .iter()
        .filter(|chain| {
            chain
                .iter()
                .any(|choice| choice.kind == ChoiceKind::UnitWithHeader)
        })
        .count()
}

/// Tries complete lockstep choices without exposing partial conflict members.
fn admit_parent(
    candidates: &[SelectionCandidate<'_>],
    selected: &mut SelectionState,
    unit: &SelectionUnit,
    choices: &[Vec<DeliveryChoice>],
    budget: &SelectionBudget<'_>,
) -> Result<Option<usize>, EvidenceError> {
    let tiers = choices.iter().map(Vec::len).max().unwrap_or(0);
    let mut order: Vec<_> = (0..tiers).collect();
    if budget.parent_chain_order == ParentChainOrder::LargestFittingParent {
        order.reverse();
    }
    for tier in order {
        let trial = parent_trial(&selected.spans, unit, choices, tier);
        if try_spans(candidates, selected, trial, budget)? {
            return Ok(Some(tier));
        }
    }
    Ok(None)
}

/// Preserves the full-section and relevant-block strategies, including growth order.
fn select_legacy(
    candidates: &[SelectionCandidate<'_>],
    unit: &SelectionUnit,
    selected: &mut SelectionState,
    budget: &SelectionBudget<'_>,
    omissions: &mut OmissionStatus,
) -> Result<bool, EvidenceError> {
    if budget.expansion == ExpansionMode::FullSection {
        let full = candidate_spans(candidates, &unit.candidates, |candidate| {
            Ok(candidate.expansion.extent)
        })?;
        let mut trial_spans = selected.spans.clone();
        trial_spans.extend(full);
        if try_spans(candidates, selected, trial_spans, budget)? {
            return Ok(false);
        }
    }

    if budget.expansion == ExpansionMode::RelevantBlocks {
        let mut trial_spans = selected.spans.clone();
        let relevant = candidate_spans(candidates, &unit.candidates, relevant_span)?;
        let prefixes = table_prefixes(
            candidates,
            &relevant
                .iter()
                .filter_map(|(index, choice)| choice.context.first().map(|span| (*index, *span)))
                .collect(),
        );
        trial_spans.extend(relevant);
        if try_spans(candidates, selected, trial_spans, budget)? {
            return Ok(true);
        }
        // Each relevant window lies within its mandatory window, which cannot fit either.
        omissions.table_prefix_omissions += prefixes;
        omit(omissions, unit);
        return Ok(false);
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
        omit(omissions, unit);
        return Ok(false);
    }

    accept_trial(candidates, selected, trial_spans, rendered, budget.control)?;
    for index in &unit.candidates {
        add_optional_siblings(candidates, *index, selected, budget)?;
    }
    Ok(false)
}
