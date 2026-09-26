//! Judging one question: ranking its bundle's passages, finding the sections
//! it expects among them, and classifying its failures.

use super::report::{Expected, Failure, FailureClass, QuestionResult};
use crate::suite::Question;
use maestro_kernel::evidence::{Bundle, Passage, RouteStatus};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

/// The cut-offs of the metrics: 5 for Recall@5, 10 for Recall@10, MRR@10 and
/// nDCG@10.
const CUTOFFS: [u32; 2] = [5, 10];

/// The name a bundle gives the reranker among its routes: it orders what the
/// routes found, and finds nothing itself.
const RERANKER: &str = "rerank";

/// What `question`, whose expected sections resolve to the IDs `expected`,
/// got from `bundle`, retrieved in `latency_us` microseconds: the rank of
/// each expected section, and, for an answerable question, a failure at each
/// cut-off none ranks within.
pub(super) fn judge(
    question: &Question,
    expected: &[String],
    bundle: &Bundle,
    latency_us: u32,
) -> QuestionResult {
    let ranked = ranked(bundle);
    let expected = expected
        .iter()
        .map(|section_id| Expected {
            section_id: section_id.clone(),
            rank: ranked
                .iter()
                .position(|passage| passage.section_id.as_ref() == Some(section_id))
                .and_then(|index| u32::try_from(index + 1).ok()),
        })
        .collect();
    let mut result = QuestionResult {
        id: question.id.clone(),
        answerable: question.answerable,
        expected,
        passages: bundle.passages.len(),
        latency_us,
        degraded: bundle
            .routes
            .values()
            .any(|status| matches!(status, RouteStatus::Unavailable(_))),
        failures: Vec::new(),
    };
    if question.answerable {
        result.failures = failures(&result, bundle);
    }
    result
}

/// The passages of `bundle` in rank order, since the bundle lists them in
/// reading order: those its trace scores by score, highest first, then those
/// it does not; passages of equal score, and unscored ones, by number, lowest
/// first. A score that is not a finite number ranks as none: a bundle refuses
/// one when it is read or written, but not when a retrieval hands it over in
/// the process.
fn ranked(bundle: &Bundle) -> Vec<&Passage> {
    let scores: BTreeMap<u32, f64> = bundle
        .trace
        .iter()
        .filter_map(|entry| Some((entry.n, entry.score.filter(|score| score.is_finite())?)))
        .collect();
    let mut ranked: Vec<(&Passage, Option<f64>)> = bundle
        .passages
        .iter()
        .map(|passage| (passage, scores.get(&passage.n).copied()))
        .collect();
    ranked.sort_by(|(left, left_score), (right, right_score)| {
        by_score(*left_score, *right_score).then(left.n.cmp(&right.n))
    });
    ranked.into_iter().map(|(passage, _)| passage).collect()
}

/// How two passages' scores, each finite if any, order them: the higher
/// first, a score before none, and equal scores, or none, alike.
fn by_score(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.partial_cmp(&left).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// The failures of `result`, an answerable question's, one at each cut-off
/// no expected section ranks within, each naming the routes of `bundle` that
/// missed and those that could not run.
fn failures(result: &QuestionResult, bundle: &Bundle) -> Vec<Failure> {
    let best = result.best_rank();
    let class = if best.is_some() {
        FailureClass::Misranked
    } else {
        FailureClass::NotRetrieved
    };
    let missed_by = missed_by(result, bundle);
    let unavailable: BTreeMap<String, String> = bundle
        .routes
        .iter()
        .filter_map(|(name, status)| match status {
            RouteStatus::Unavailable(reason) => Some((name.clone(), reason.clone())),
            RouteStatus::Ok => None,
        })
        .collect();
    CUTOFFS
        .into_iter()
        .filter(|&cutoff| best.is_none_or(|rank| rank > cutoff))
        .map(|cutoff| Failure {
            cutoff,
            class,
            missed_by: missed_by.clone(),
            unavailable: unavailable.clone(),
        })
        .collect()
}

/// The routes of `bundle` that ran, the reranker aside, and whose trace found
/// none of the sections `result` expects, in name order.
fn missed_by(result: &QuestionResult, bundle: &Bundle) -> Vec<String> {
    let expected: BTreeSet<&str> = result
        .expected
        .iter()
        .map(|expected| expected.section_id.as_str())
        .collect();
    let holding: BTreeSet<u32> = bundle
        .passages
        .iter()
        .filter(|passage| {
            let section = passage.section_id.as_deref();
            section.is_some_and(|section| expected.contains(section))
        })
        .map(|passage| passage.n)
        .collect();
    let found: BTreeSet<&str> = bundle
        .trace
        .iter()
        .filter(|entry| holding.contains(&entry.n))
        .flat_map(|entry| entry.routes.iter().map(String::as_str))
        .collect();
    bundle
        .routes
        .iter()
        .filter(|&(name, status)| {
            *status == RouteStatus::Ok && name != RERANKER && !found.contains(name.as_str())
        })
        .map(|(name, _)| name.clone())
        .collect()
}
