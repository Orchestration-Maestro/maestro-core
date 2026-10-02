//! Observed-only proof attrition and conclusion support in the graph scorer.

use super::{
    label_types::{CheckedItem, Located, QuestionKind},
    label_validation::safe_id,
    score::{ProofScore, proof_complete, score_proofs},
};
use serde::Serialize;
use std::{
    array::from_fn,
    collections::{BTreeMap, btree_map::Entry},
    slice::from_ref,
};

/// One conclusion's citations and the answer validator's support observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConclusionObservation {
    /// Exact source coordinates actually cited.
    pub citations: Vec<Located>,
    /// Content validation, independent of citation presence.
    pub supported: bool,
}

/// One item's observed evidence stages; `None` means not observed, not empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofObservation {
    /// Stable item ID.
    pub id: String,
    /// Candidate, pre-fusion, post-packing and final-wire coordinates, in order.
    pub stages: [Option<Vec<Located>>; 4],
    /// Conclusions actually observed; absent when the answer stage was not observed.
    pub conclusions: Option<Vec<ConclusionObservation>>,
}

/// A diagnostic numerator, its observed denominator and missing observations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DiagnosticRatio {
    /// Credited or lost observations, according to the measure.
    pub count: usize,
    /// Observations eligible for this measure.
    pub of: usize,
    /// Items or known conclusions not observed for this measure.
    pub unobserved: usize,
}

/// Complete-proof recall and attrition at one evidence stage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ProofStageScore {
    /// Complete proofs over items observed at this stage.
    pub complete: DiagnosticRatio,
    /// Lost proofs over preceding complete items observed at both stages.
    pub attrition: DiagnosticRatio,
}

/// Conclusion diagnostics never equate a present citation with support.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ConclusionScore {
    /// Conclusions with any citation over observed conclusions.
    pub citation_presence: DiagnosticRatio,
    /// Conclusions with one complete proof both cited and delivered.
    pub citation_support: DiagnosticRatio,
    /// Conclusions lacking validated content and a complete cited/delivered proof.
    pub unsupported: DiagnosticRatio,
    /// Answerable items with no conclusion observation; their conclusion count is unknown.
    pub unobserved_items: usize,
}

/// All graph proof and answer diagnostics from the one scorer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphScore {
    /// Candidate, pre-fusion, post-packing and final-wire stage diagnostics.
    pub stages: [ProofStageScore; 4],
    /// Conclusion diagnostics with observed denominators.
    pub conclusions: ConclusionScore,
    /// Final delivered complete-proof recall over all answerable families.
    pub final_wire: ProofScore,
}

/// Invalid observations, with a fixed code and safe item ID only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticError {
    /// Fixed refusal code.
    pub code: &'static str,
    /// Safe item ID, absent for an unsafe identifier.
    pub item: Option<String>,
}

/// Score observed proof stages and conclusions using the shared complete-proof rule.
///
/// # Errors
/// Refuses duplicate, unknown or unsafe items and inconsistent proof-stage evidence.
pub fn score_graph(
    items: &[CheckedItem],
    observations: &[ProofObservation],
) -> Result<GraphScore, DiagnosticError> {
    let by_id = index_observations(items, observations)?;
    let delivered = observations
        .iter()
        .filter_map(|row| {
            row.stages[3]
                .as_ref()
                .map(|anchors| (row.id.clone(), anchors.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut score = GraphScore {
        stages: [ProofStageScore::default(); 4],
        conclusions: ConclusionScore::default(),
        final_wire: score_proofs(items, &delivered),
    };
    for item in items
        .iter()
        .filter(|item| item.kind != QuestionKind::Unanswerable)
    {
        let observation = by_id.get(item.id.as_str()).copied();
        let stages = from_fn::<_, 4, _>(|index| {
            observation
                .and_then(|row| row.stages.get(index).and_then(Option::as_ref))
                .map(|anchors| proof_complete(&item.proofs, anchors))
        });
        validate_stages(item, stages)?;
        include_stages(&mut score.stages, stages);
        include_conclusions(&mut score.conclusions, item, observation);
    }
    Ok(score)
}

/// Count observed-only completeness and attrition between co-observed stages.
fn include_stages(scores: &mut [ProofStageScore; 4], stages: [Option<bool>; 4]) {
    for (index, (score, current)) in scores.iter_mut().zip(stages).enumerate() {
        if let Some(complete) = current {
            score.complete.of += 1;
            score.complete.count += usize::from(complete);
        } else {
            score.complete.unobserved += 1;
        }
        let previous = if index == 0 {
            Some(true)
        } else {
            stages.get(index - 1).copied().flatten()
        };
        match (previous, current) {
            (Some(true), Some(complete)) => {
                score.attrition.of += 1;
                score.attrition.count += usize::from(!complete);
            }
            (Some(false), Some(_)) => {}
            _ => score.attrition.unobserved += 1,
        }
    }
}

/// Pair observations exactly once, refusing unsafe identity text before returning it.
fn index_observations<'a>(
    items: &[CheckedItem],
    rows: &'a [ProofObservation],
) -> Result<BTreeMap<&'a str, &'a ProofObservation>, DiagnosticError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        if !safe_id(&row.id) {
            return Err(DiagnosticError {
                code: "graph_observation_unsafe_id",
                item: None,
            });
        }
        if !items.iter().any(|item| item.id == row.id) {
            return Err(refusal("graph_observation_unknown", &row.id));
        }
        match indexed.entry(row.id.as_str()) {
            Entry::Vacant(entry) => {
                entry.insert(row);
            }
            Entry::Occupied(_) => return Err(refusal("graph_observation_duplicate", &row.id)),
        }
    }
    Ok(indexed)
}

/// A later observed complete proof cannot recover an earlier observed lost proof.
fn validate_stages(item: &CheckedItem, stages: [Option<bool>; 4]) -> Result<(), DiagnosticError> {
    let mut incomplete = false;
    for complete in stages.into_iter().flatten() {
        if incomplete && complete {
            return Err(refusal("graph_proof_inconsistent", &item.id));
        }
        incomplete |= !complete;
    }
    Ok(())
}

/// Fixed safe refusal; never includes evidence or conclusion text.
fn refusal(code: &'static str, id: &str) -> DiagnosticError {
    DiagnosticError {
        code,
        item: safe_id(id).then(|| id.to_owned()),
    }
}

/// Citation coverage and unsupported content use co-observed wire/conclusion evidence.
fn include_conclusions(
    score: &mut ConclusionScore,
    item: &CheckedItem,
    row: Option<&ProofObservation>,
) {
    let Some(conclusions) = row.and_then(|row| row.conclusions.as_ref()) else {
        score.unobserved_items += 1;
        return;
    };
    for conclusion in conclusions {
        score.citation_presence.of += 1;
        score.citation_presence.count += usize::from(!conclusion.citations.is_empty());
        let Some(wire) = row.and_then(|row| row.stages[3].as_ref()) else {
            score.citation_support.unobserved += 1;
            score.unsupported.unobserved += 1;
            continue;
        };
        let cited_proof = item.proofs.iter().any(|proof| {
            proof_complete(from_ref(proof), &conclusion.citations)
                && proof_complete(from_ref(proof), wire)
        });
        score.citation_support.of += 1;
        score.citation_support.count += usize::from(cited_proof);
        score.unsupported.of += 1;
        score.unsupported.count += usize::from(!(cited_proof && conclusion.supported));
    }
}
