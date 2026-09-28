//! Frozen evaluation run gates over construction, proof, retrieval and latency evidence.

use super::score::{FamilyProof, ScoreConstruction, proof_gain};
use crate::eval::bootstrap::estimates;
use crate::{search::SearchConfiguration, suite::Suite};
use maestro_kernel::artifact::Digest;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

/// Graph route selected by an evaluation rung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphRoute {
    /// Passage-only baseline.
    None,
    /// Graph enabled.
    Enabled,
}
/// One rung'sample frozen retrieval definition.
#[derive(Debug, Clone, PartialEq)]
pub struct RungDefinition {
    /// S1 search routes/weights.
    pub configuration: SearchConfiguration,
    /// Frozen reranker identity.
    pub reranker: Option<Digest>,
    /// Graph route.
    pub graph: GraphRoute,
}
/// Per-question retrieval outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionRetrieval {
    /// Stable question ID.
    pub id: String,
    /// A relevant item occurs in top ten.
    pub hit_at_10: bool,
    /// Reciprocal rank in top ten.
    pub reciprocal_rank: f64,
}
/// Whether one unanswerable question was refused correctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalOutcome {
    /// Correct refusal.
    Refused,
    /// Answered despite unanswerability.
    Answered,
    /// Failed or timed out; no refusal credit.
    Failed,
    /// Timed out; no refusal credit.
    TimedOut,
}
/// One evaluated rung in one run.
#[derive(Debug, Clone, PartialEq)]
pub struct RungEvidence {
    /// Run number.
    pub run: u32,
    /// Frozen configuration.
    pub definition: RungDefinition,
    /// Complete-proof outcomes.
    pub proofs: Vec<FamilyProof>,
    /// Graph retrieval results.
    pub graph_retrieval: Vec<QuestionRetrieval>,
    /// Supported answers.
    pub supported_answers: usize,
    /// Paired ctm retrieval comparator.
    pub ctm_retrieval: Vec<QuestionRetrieval>,
    /// Unanswerable question results.
    pub refusals: Vec<RefusalOutcome>,
}
/// Construction and all rung evidence for one repeat.
#[derive(Debug, Clone, PartialEq)]
pub struct RunEvidence {
    /// Distinct repeat identifier.
    pub run: u32,
    /// Required passage-only rung.
    pub passage_only: RungEvidence,
    /// Required graph-plus-passage rung.
    pub pairing: RungEvidence,
    /// Reviewed construction metrics.
    pub construction: ScoreConstruction,
    /// Invalid delivered commands.
    pub inexact_commands: usize,
    /// Warm/cold/unavailable operation timings.
    pub latency: Vec<LatencySample>,
}
/// Cohort of baseline measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cohort {
    /// Warm state.
    Warm,
    /// Cold start.
    Cold,
    /// Could not measure.
    Unavailable,
}
/// Operation latency category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Graph retrieval route (not a tool).
    Graph,
    /// Graph neighbors tool.
    Neighbors,
    /// Graph path tool.
    Path,
    /// Entity resolution tool.
    EntityResolve,
    /// Evidence trace tool.
    EvidenceTrace,
    /// Full search.
    Search,
    /// Complete answer.
    Ask,
}
/// Latency result; absent elapsed time is baseline failure in warm cohort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencySample {
    /// Measurement cohort.
    pub cohort: Cohort,
    /// Operation measured.
    pub operation: Operation,
    /// Duration, absent for timeout.
    pub elapsed: Option<Duration>,
}
/// Failed gate category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Pairing lacks proof-gain threshold.
    ProofGain,
    /// Not the same run comparator.
    SameRunComparator,
    /// Frozen passage/pairing definitions differ.
    PassageDefinition,
    /// Construction precision failed.
    RelationPrecision,
    /// Quotes or answers are inexact.
    Exactness,
    /// Unanswerable refusal rate below 80%.
    Refusal,
    /// Retrieval/supported-answer point estimate regressed.
    GraphRetrieval,
    /// ctm retrieval regressed.
    CtmRetrieval,
    /// Warm p95 limits failed.
    Latency,
}
/// Metrics and deterministic gate verdict for one run.
#[derive(Debug, Clone, PartialEq)]
pub struct RunVerdict {
    /// Repeat number.
    pub run: u32,
    /// Paired ctm recall diagnostic interval.
    pub ctm_recall_interval: Option<[f64; 2]>,
    /// Gate list.
    pub passed: bool,
    /// Gates that failed.
    failed: Vec<Gate>,
}
impl RunVerdict {
    /// Gates not met by this run.
    #[must_use]
    pub fn failed_gates(&self) -> Vec<Gate> {
        self.failed.clone()
    }
    /// Whether this run meets every gate.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.passed
    }
}
/// Overall three-run decision.
#[derive(Debug, Clone, PartialEq)]
pub struct RunsVerdict {
    /// Every independently judged run.
    pub runs: Vec<RunVerdict>,
    /// Exactly three distinct passing runs.
    pub passed: bool,
}
/// Apply the frozen run gates against the answerable IDs of each frozen suite.
/// Every repeat must pass independently; unanswerables belong only to refusals.
#[must_use]
pub fn judge_runs(evidence: &[RunEvidence], graph_suite: &Suite, ctm_suite: &Suite) -> RunsVerdict {
    let graph_ids = answerable_ids(graph_suite);
    let ctm_ids = answerable_ids(ctm_suite);
    let runs: Vec<_> = evidence
        .iter()
        .map(|run| judge_run(run, &graph_ids, &ctm_ids))
        .collect();
    let unique = runs
        .iter()
        .map(|result| result.run)
        .collect::<BTreeSet<_>>()
        .len()
        == 3;
    RunsVerdict {
        passed: runs.len() == 3 && unique && runs.iter().all(|result| result.passed),
        runs,
    }
}
/// Judge one repeat independently.
fn judge_run(
    evidence: &RunEvidence,
    graph_ids: &BTreeSet<&str>,
    ctm_ids: &BTreeSet<&str>,
) -> RunVerdict {
    let mut failed = Vec::new();
    if evidence.passage_only.run != evidence.run || evidence.pairing.run != evidence.run {
        failed.push(Gate::SameRunComparator);
    }
    let defaults = SearchConfiguration::default();
    let baseline = &evidence.passage_only.definition;
    if baseline.graph != GraphRoute::None
        || baseline.configuration != defaults
        || baseline.reranker != evidence.pairing.definition.reranker
        || evidence.pairing.definition.configuration != defaults
        || evidence.pairing.definition.graph != GraphRoute::Enabled
    {
        failed.push(Gate::PassageDefinition);
    }
    match proof_gain(&evidence.passage_only.proofs, &evidence.pairing.proofs, 0) {
        Ok(gain) if gain.passed && gain.pairs == 80 => {}
        _ => failed.push(Gate::ProofGain),
    }
    if !evidence.construction.precision_passed {
        failed.push(Gate::RelationPrecision);
    }
    if !evidence.construction.exact || evidence.inexact_commands > 0 {
        failed.push(Gate::Exactness);
    }
    if evidence.pairing.refusals.len() != 20
        || evidence
            .pairing
            .refusals
            .iter()
            .filter(|result| **result == RefusalOutcome::Refused)
            .count()
            < 16
    {
        failed.push(Gate::Refusal);
    }
    if !non_regression(
        graph_ids,
        &evidence.passage_only.graph_retrieval,
        &evidence.pairing.graph_retrieval,
    ) || evidence.pairing.supported_answers < evidence.passage_only.supported_answers
    {
        failed.push(Gate::GraphRetrieval);
    }
    let interval = paired_interval(
        ctm_ids,
        &evidence.passage_only.ctm_retrieval,
        &evidence.pairing.ctm_retrieval,
    );
    if !non_regression(
        ctm_ids,
        &evidence.passage_only.ctm_retrieval,
        &evidence.pairing.ctm_retrieval,
    ) {
        failed.push(Gate::CtmRetrieval);
    }
    if !latency_pass(&evidence.latency) {
        failed.push(Gate::Latency);
    }
    RunVerdict {
        run: evidence.run,
        ctm_recall_interval: interval,
        passed: failed.is_empty(),
        failed,
    }
}
/// Frozen retrieval metrics include every answerable question and no refusals.
fn answerable_ids(suite: &Suite) -> BTreeSet<&str> {
    suite
        .questions
        .iter()
        .filter(|question| question.answerable)
        .map(|question| question.id.as_str())
        .collect()
}
/// Rows must cover the frozen answerable population exactly once.
fn complete_population(rows: &[QuestionRetrieval], expected: &BTreeSet<&str>) -> bool {
    !expected.is_empty()
        && rows.len() == expected.len()
        && rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<BTreeSet<_>>()
            == *expected
}
/// Compare paired retrieval point estimates.
fn non_regression(
    expected: &BTreeSet<&str>,
    base: &[QuestionRetrieval],
    candidate: &[QuestionRetrieval],
) -> bool {
    if !complete_population(base, expected) || !complete_population(candidate, expected) {
        return false;
    }
    let mut baseline = base.to_vec();
    let mut paired = candidate.to_vec();
    baseline.sort_by(|x, y| x.id.cmp(&y.id));
    paired.sort_by(|x, y| x.id.cmp(&y.id));
    let recall_a = baseline.iter().filter(|x| x.hit_at_10).count();
    let recall_b = paired.iter().filter(|x| x.hit_at_10).count();
    let mrr_a: f64 = baseline.iter().map(|x| x.reciprocal_rank).sum();
    let mrr_b: f64 = paired.iter().map(|x| x.reciprocal_rank).sum();
    recall_b >= recall_a && mrr_b >= mrr_a
}
/// Compute the diagnostic paired recall interval with S1 statistics.
fn paired_interval(
    expected: &BTreeSet<&str>,
    base: &[QuestionRetrieval],
    candidate: &[QuestionRetrieval],
) -> Option<[f64; 2]> {
    if !complete_population(base, expected) || !complete_population(candidate, expected) {
        return None;
    }
    let mut baseline = base.to_vec();
    let mut paired = candidate.to_vec();
    baseline.sort_by(|x, y| x.id.cmp(&y.id));
    paired.sort_by(|x, y| x.id.cmp(&y.id));
    let deltas: Vec<f64> = baseline
        .iter()
        .zip(&paired)
        .map(|(x, y)| f64::from(u8::from(y.hit_at_10)) - f64::from(u8::from(x.hit_at_10)))
        .collect();
    let estimate = estimates(&vec![true; deltas.len()], 0, |indices| {
        let (sum, count) = indices
            .iter()
            .filter_map(|index| deltas.get(*index))
            .fold((0.0, 0.0), |(sum, count), value| (sum + value, count + 1.0));
        BTreeMap::from([((), sum / count)])
    })
    .remove(&())?;
    Some([estimate.low, estimate.high])
}
/// Apply warm nearest-rank p95 limits; cold samples are diagnostic only.
fn latency_pass(samples: &[LatencySample]) -> bool {
    for (op, limit) in [
        (Operation::Graph, Duration::from_millis(500)),
        (Operation::Neighbors, Duration::from_millis(500)),
        (Operation::Path, Duration::from_millis(500)),
        (Operation::EntityResolve, Duration::from_millis(500)),
        (Operation::EvidenceTrace, Duration::from_millis(500)),
        (Operation::Search, Duration::from_millis(2500)),
        (Operation::Ask, Duration::from_secs(10)),
    ] {
        let mut values: Vec<_> = samples
            .iter()
            .filter(|sample| sample.cohort == Cohort::Warm && sample.operation == op)
            .map(|sample| sample.elapsed)
            .collect();
        if values.is_empty() || values.iter().any(Option::is_none) {
            return false;
        }
        values.sort_by_key(|value| *value);
        let index = values
            .len()
            .saturating_mul(95)
            .div_ceil(100)
            .saturating_sub(1);
        if values.get(index).copied().flatten().is_none_or(|value| {
            if matches!(op, Operation::Search | Operation::Ask) {
                value >= limit
            } else {
                value > limit
            }
        }) {
            return false;
        }
    }
    true
}
