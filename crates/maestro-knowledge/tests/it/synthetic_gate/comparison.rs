//! Deterministic, exact comparison of complete public synthetic runs.

use maestro_knowledge::eval::{Estimate, QuestionResult, Report};
use std::collections::BTreeMap;

const QUESTIONS: usize = 56;
const ANSWERABLE: usize = 48;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum Refusal {
    Identity,
    Incomplete,
    Degraded,
    MissingMetric(&'static str),
    InvalidMetric(&'static str),
    Regression(&'static str),
}

#[derive(Clone, Copy)]
enum Direction {
    Higher,
    Lower,
}

/// Refuses different run identities, incomplete/degraded reports, invalid
/// metrics, and any deterministic regression. Timing is intentionally ignored.
pub(super) fn require_no_regression(baseline: &Report, candidate: &Report) -> Result<(), Refusal> {
    if baseline.schema != candidate.schema
        || baseline.suite != candidate.suite
        || baseline.suite_digest != candidate.suite_digest
        || baseline.collection != candidate.collection
        || baseline.profiles != candidate.profiles
        || baseline.seed != candidate.seed
    {
        return Err(Refusal::Identity);
    }
    validate_report(baseline)?;
    validate_report(candidate)?;
    let before = by_id(baseline)?;
    let after = by_id(candidate)?;
    if before.keys().ne(after.keys())
        || before.iter().any(|(id, question)| {
            after
                .get(id)
                .is_none_or(|other| question.answerable != other.answerable)
        })
    {
        return Err(Refusal::Incomplete);
    }

    let metrics = [
        (
            "recall@5",
            baseline.metrics.recall_at_5,
            candidate.metrics.recall_at_5,
            Direction::Higher,
        ),
        (
            "recall@10",
            baseline.metrics.recall_at_10,
            candidate.metrics.recall_at_10,
            Direction::Higher,
        ),
        (
            "mrr@10",
            baseline.metrics.mrr_at_10,
            candidate.metrics.mrr_at_10,
            Direction::Higher,
        ),
        (
            "ndcg@10",
            baseline.metrics.ndcg_at_10,
            candidate.metrics.ndcg_at_10,
            Direction::Higher,
        ),
        (
            "no_answer_accuracy",
            baseline.metrics.no_answer_accuracy,
            candidate.metrics.no_answer_accuracy,
            Direction::Higher,
        ),
        (
            "false_abstentions",
            baseline.metrics.false_abstentions,
            candidate.metrics.false_abstentions,
            Direction::Lower,
        ),
    ];
    for (name, before, after, direction) in metrics {
        let before = estimate(name, before)?;
        let after = estimate(name, after)?;
        let regressed = match direction {
            Direction::Higher => after.value < before.value,
            Direction::Lower => after.value > before.value,
        };
        if regressed {
            return Err(Refusal::Regression(name));
        }
    }
    Ok(())
}

/// Requires the exact synthetic suite accounting and no unexpected route degradation.
fn validate_report(report: &Report) -> Result<(), Refusal> {
    if report.questions.len() != QUESTIONS {
        return Err(Refusal::Incomplete);
    }
    if report.degraded_searches != 0 || report.questions.iter().any(|question| question.degraded) {
        return Err(Refusal::Degraded);
    }
    let answerable = report
        .questions
        .iter()
        .filter(|question| question.answerable)
        .count();
    if answerable != ANSWERABLE
        || report
            .questions
            .iter()
            .any(|question| question.expected.is_empty() == question.answerable)
    {
        return Err(Refusal::Incomplete);
    }
    Ok(())
}

/// The questions by ID, refusing duplicate IDs.
fn by_id(report: &Report) -> Result<BTreeMap<&str, &QuestionResult>, Refusal> {
    let mut questions = BTreeMap::new();
    for question in &report.questions {
        if questions.insert(question.id.as_str(), question).is_some() {
            return Err(Refusal::Incomplete);
        }
    }
    Ok(questions)
}

/// A present finite metric estimate within its declared unit interval.
fn estimate(name: &'static str, estimate: Option<Estimate>) -> Result<Estimate, Refusal> {
    let Some(estimate) = estimate else {
        return Err(Refusal::MissingMetric(name));
    };
    let values = [estimate.low, estimate.value, estimate.high];
    if values
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        || estimate.low > estimate.value
        || estimate.value > estimate.high
    {
        return Err(Refusal::InvalidMetric(name));
    }
    Ok(estimate)
}
