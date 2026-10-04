//! Per-item answer attempts, separate from retrieval evidence.

use crate::suite::Suite;

/// Closed answer outcome; operational failures earn no credit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerOutcome {
    /// A conclusion supported by complete cited evidence.
    Supported,
    /// An unsupported conclusion, even when a citation is present.
    Unsupported,
    /// Correctly refused an unanswerable question.
    Refused,
    /// Request failed.
    Failed,
    /// Request exceeded its deadline.
    TimedOut,
}

/// One answered request, paired by the frozen suite's item ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionAnswer {
    /// Stable item ID, never question or answer text.
    pub id: String,
    /// Final outcome of the answered request.
    pub outcome: AnswerOutcome,
}

/// Count credit only on the relevant frozen population, once per item.
pub(super) fn credited(suite: &Suite, rows: &[QuestionAnswer], answerable: bool) -> usize {
    let outcome = if answerable {
        AnswerOutcome::Supported
    } else {
        AnswerOutcome::Refused
    };
    suite
        .questions
        .iter()
        .filter(|question| {
            question.answerable == answerable
                && rows.iter().filter(|row| row.id == question.id).count() == 1
                && rows
                    .iter()
                    .any(|row| row.id == question.id && row.outcome == outcome)
        })
        .count()
}

/// Compare the two Golden point estimates on their fixed 84/16 denominators.
pub(super) fn golden_non_regression(
    suite: &Suite,
    base: &[QuestionAnswer],
    candidate: &[QuestionAnswer],
) -> [bool; 2] {
    [true, false].map(|answerable| {
        let required = if answerable { 84 } else { 16 };
        suite
            .questions
            .iter()
            .filter(|question| question.answerable == answerable)
            .count()
            == required
            && credited(suite, candidate, answerable) >= credited(suite, base, answerable)
    })
}
