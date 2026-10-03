//! Construction and complete-proof metrics for graph evaluation.

use super::label_types::{CheckedItem, Located, QuestionKind};
use crate::eval::bootstrap::estimates;
use std::collections::{BTreeMap, BTreeSet};

/// Fixed seed used by the frozen paired bootstrap.
pub const GAIN_SEED: u64 = 0;

/// Whether a required proof is completely present in delivered evidence.
#[must_use]
pub fn proof_complete(proofs: &[Vec<Located>], delivered: &[Located]) -> bool {
    proofs.iter().any(|proof| {
        !proof.is_empty()
            && proof.iter().all(|required| {
                delivered.iter().any(|actual| {
                    actual.revision_id == required.revision_id
                        && actual.span[0] <= required.span[0]
                        && actual.span[1] >= required.span[1]
                })
            })
    })
}

/// Result for a single independent question family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyProof {
    /// Stable independent family.
    pub family: String,
    /// Whether any complete accepted proof was delivered.
    pub complete: bool,
}

/// Per-question complete-proof recall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofScore {
    /// Complete answerable families.
    pub complete: usize,
    /// Number of answerable families.
    pub of: usize,
    /// Each family's result once.
    pub families: Vec<FamilyProof>,
}

/// Score answerable questions once each; unanswerables are excluded.
#[must_use]
pub fn score_proofs(
    items: &[CheckedItem],
    delivered: &BTreeMap<String, Vec<Located>>,
) -> ProofScore {
    let families: Vec<_> = items
        .iter()
        .filter(|item| item.kind != QuestionKind::Unanswerable)
        .map(|item| FamilyProof {
            family: item.family.clone(),
            complete: proof_complete(
                &item.proofs,
                delivered.get(&item.id).map_or(&[], Vec::as_slice),
            ),
        })
        .collect();
    ProofScore {
        complete: families.iter().filter(|family| family.complete).count(),
        of: families.len(),
        families,
    }
}

/// A paired-score validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainError {
    /// Empty sample.
    Empty,
    /// Duplicate family.
    DuplicateFamily,
    /// Baseline and candidate family sets differ.
    UnpairedFamily,
}

/// Paired complete-proof gain and bootstrap gate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GainScore {
    /// Paired count.
    pub pairs: usize,
    /// Candidate-only successes.
    pub wins: usize,
    /// Baseline-only successes.
    pub losses: usize,
    /// Candidate minus baseline.
    pub observed: f64,
    /// 50th ordered bootstrap value.
    pub low: f64,
    /// 1,950th ordered bootstrap value.
    pub high: f64,
    /// Gate decision.
    pub passed: bool,
}

/// Pair and bootstrap binary family outcomes with replacement.
///
/// # Errors
/// Rejects empty, duplicate or unpaired family sets.
pub fn proof_gain(
    baseline: &[FamilyProof],
    candidate: &[FamilyProof],
    seed: u64,
) -> Result<GainScore, GainError> {
    if baseline.is_empty() || candidate.is_empty() {
        return Err(GainError::Empty);
    }
    let mut base = BTreeMap::new();
    let mut cand = BTreeMap::new();
    for row in baseline {
        if base.insert(&row.family, row.complete).is_some() {
            return Err(GainError::DuplicateFamily);
        }
    }
    for row in candidate {
        if cand.insert(&row.family, row.complete).is_some() {
            return Err(GainError::DuplicateFamily);
        }
    }
    if base.keys().collect::<BTreeSet<_>>() != cand.keys().collect::<BTreeSet<_>>() {
        return Err(GainError::UnpairedFamily);
    }
    let deltas: Vec<f64> = base
        .values()
        .zip(cand.values())
        .map(|(before, after)| f64::from(u8::from(*after)) - f64::from(u8::from(*before)))
        .collect();
    let wins = deltas.iter().filter(|&&delta| delta > 0.0).count();
    let losses = deltas.iter().filter(|&&delta| delta < 0.0).count();
    let estimate = estimates(&vec![true; deltas.len()], seed, |indices| {
        let (sum, count) = indices
            .iter()
            .filter_map(|index| deltas.get(*index))
            .fold((0.0, 0.0), |(sum, count), value| (sum + value, count + 1.0));
        BTreeMap::from([((), sum / count)])
    })
    .remove(&())
    .ok_or(GainError::Empty)?;
    let observed = estimate.value;
    let low = estimate.low;
    let high = estimate.high;
    Ok(GainScore {
        pairs: deltas.len(),
        wins,
        losses,
        observed,
        low,
        high,
        passed: observed >= 0.05 && low > 0.0,
    })
}

/// Reviewed construction disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimVerdict {
    /// Review accepts claim.
    Correct,
    /// Review rejects claim.
    Incorrect,
    /// Review is unresolved.
    Unresolved,
}
/// Independent claim review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimReview {
    /// Claim identifier.
    pub claim_id: String,
    /// Review disposition.
    pub verdict: ClaimVerdict,
    /// Exact source quote check.
    pub quote_exact: bool,
}
/// Predicate triple used by the construction coverage metric.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Triple {
    /// Subject.
    pub subject: String,
    /// Relation.
    pub predicate: String,
    /// Object.
    pub object: String,
}
/// Construction metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreConstruction {
    /// Correct accepted claims.
    pub correct: usize,
    /// All accepted/rejected claims reviewed.
    pub accepted: usize,
    /// Unresolved reviews.
    pub unresolved: usize,
    /// Inexact quotes across all construction reviews.
    pub inexact_quotes: usize,
    /// Gold relations found.
    pub gold_found: usize,
    /// Gold relation count.
    pub gold: usize,
    /// ≥95% semantic precision and no unresolved reviews.
    pub precision_passed: bool,
    /// All construction review quotes exact.
    pub exact: bool,
}
/// Invalid construction review population.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionError {
    /// A claim has more than one review receipt.
    DuplicateClaim,
}
/// Score reviewed relation construction and required gold coverage.
///
/// # Errors
/// Rejects duplicate claim IDs, including identical review receipts.
pub fn score_construction(
    reviews: &[ClaimReview],
    gold: &[Triple],
    built: &[Triple],
) -> Result<ScoreConstruction, ConstructionError> {
    let ids: BTreeSet<_> = reviews.iter().map(|review| &review.claim_id).collect();
    if ids.len() != reviews.len() {
        return Err(ConstructionError::DuplicateClaim);
    }
    let accepted = reviews
        .iter()
        .filter(|review| review.verdict != ClaimVerdict::Unresolved)
        .count();
    let correct = reviews
        .iter()
        .filter(|review| review.verdict == ClaimVerdict::Correct)
        .count();
    let unresolved = reviews
        .iter()
        .filter(|review| review.verdict == ClaimVerdict::Unresolved)
        .count();
    let inexact_quotes = reviews.iter().filter(|review| !review.quote_exact).count();
    let gold_found = gold
        .iter()
        .filter(|gold_triple| built.contains(gold_triple))
        .collect::<BTreeSet<_>>()
        .len();
    let precision_passed = accepted > 0 && correct * 100 >= accepted * 95 && unresolved == 0;
    Ok(ScoreConstruction {
        correct,
        accepted,
        unresolved,
        inexact_quotes,
        gold_found,
        gold: gold.len(),
        precision_passed,
        exact: inexact_quotes == 0,
    })
}
