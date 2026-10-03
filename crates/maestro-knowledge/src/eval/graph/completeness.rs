//! Exact answered-request populations; diagnostics contain counts and safe IDs only.

use super::{answers::QuestionAnswer, label_validation::safe_id};
use crate::suite::Suite;
use std::collections::{BTreeMap, BTreeSet};

/// Counts and item-only diagnostics for answered-request completeness.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestCompleteness {
    /// Required answered requests, including every arm and repeat.
    pub required: usize,
    /// Observed request rows, including duplicates and unknown items.
    pub observed: usize,
    /// Absent item IDs; an ID may recur across arms or suites.
    pub missing_ids: Vec<String>,
    /// Duplicate or unknown safe item IDs.
    pub rejected_ids: Vec<String>,
    /// Unsafe IDs withheld from diagnostics.
    pub unsafe_ids: usize,
    /// Exact suite populations and one row per item.
    pub passed: bool,
}

/// Validate the graph 200+20 and Golden 84+16 population for every supplied arm.
pub(super) fn request_completeness(
    graph: &Suite,
    golden: &Suite,
    arms: &[(&[QuestionAnswer], &[QuestionAnswer])],
) -> RequestCompleteness {
    let mut score = RequestCompleteness {
        required: 320 * arms.len(),
        passed: population(graph, 200, 20) && population(golden, 84, 16),
        ..RequestCompleteness::default()
    };
    for (graph_rows, golden_rows) in arms {
        include_population(&mut score, graph, graph_rows);
        include_population(&mut score, golden, golden_rows);
    }
    score.passed &= score.required == score.observed
        && score.missing_ids.is_empty()
        && score.rejected_ids.is_empty()
        && score.unsafe_ids == 0;
    score
}

/// Suites must retain their approved counts and safe, unique item identities.
fn population(suite: &Suite, answerable: usize, unanswerable: usize) -> bool {
    suite.questions.len() == answerable + unanswerable
        && suite
            .questions
            .iter()
            .filter(|question| question.answerable)
            .count()
            == answerable
        && suite.questions.iter().all(|question| safe_id(&question.id))
        && suite
            .questions
            .iter()
            .map(|question| &question.id)
            .collect::<BTreeSet<_>>()
            .len()
            == suite.questions.len()
}

/// Include one frozen population without disclosing any text from its questions.
fn include_population(score: &mut RequestCompleteness, suite: &Suite, rows: &[QuestionAnswer]) {
    score.observed += rows.len();
    let expected: BTreeSet<_> = suite
        .questions
        .iter()
        .map(|question| question.id.as_str())
        .collect();
    let mut counts = BTreeMap::new();
    for row in rows {
        *counts.entry(row.id.as_str()).or_insert(0) += 1;
    }
    for id in &expected {
        if !counts.contains_key(id) && safe_id(id) {
            score.missing_ids.push((*id).to_owned());
        }
    }
    for (id, count) in counts {
        if !safe_id(id) {
            score.unsafe_ids += count;
        } else if count != 1 || !expected.contains(id) {
            score.rejected_ids.push(id.to_owned());
        }
    }
}
