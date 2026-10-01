//! V1 and v2 report-reader rules, shared with the v2 writer.

use super::{
    metric::{measure_attempts, measure_cohorts, measure_subgroups},
    reports::{
        CohortStatistics, Estimate, ItemStatus, MeasurementCohort, Metrics, QuestionResult, Report,
        Schema, Subgroup, SubgroupStatistics, TrialMode,
    },
};
use maestro_kernel::artifact::Digest;
use serde::de::Error as SerdeDeError;
use std::collections::{BTreeMap, BTreeSet};

impl Report {
    /// Enforces v1 compatibility, complete v2 metadata and row-derived summaries.
    pub(super) fn validate_version(&self) -> Result<(), serde_json::Error> {
        let v2_report = self.run_id.is_some()
            || self.candidate_id.is_some()
            || self.card_id.is_some()
            || self.card_digest.is_some()
            || self.manifest_digest.is_some()
            || self.planned_repetitions.is_some()
            || self.planned_warm_ups.is_some()
            || self.corpus_digest.is_some()
            || self.input_digest.is_some()
            || self.mode.is_some()
            || self.integrity_violation.is_some()
            || self.cohorts.is_some()
            || self.subgroups.is_some();
        match self.schema {
            Schema::V1 if !v2_report && self.questions.iter().all(QuestionResult::is_v1) => Ok(()),
            Schema::V1 => Err(invalid("v1 report contains v2 metadata")),
            Schema::V2
                if valid_v2_header(
                    &self.suite,
                    &self.collection,
                    &self.profiles,
                    self.run_id.as_deref(),
                    self.candidate_id.as_deref(),
                    self.card_id.as_deref(),
                    self.card_digest.as_ref(),
                    self.manifest_digest.as_ref(),
                    self.corpus_digest.as_ref(),
                    self.input_digest.as_ref(),
                    self.mode,
                    self.planned_repetitions,
                    self.planned_warm_ups,
                ) && self.integrity_violation.is_some()
                    && self.cohorts.is_some()
                    && self.questions.iter().all(QuestionResult::is_v2)
                    && self.v2_questions_are_consistent()
                    && self.v2_derived_fields_are_consistent() =>
            {
                Ok(())
            }
            Schema::V2 => Err(invalid("v2 report is missing frozen or item metadata")),
        }
    }

    /// Ensures one route, stable question labels and unique attempt rows.
    fn v2_questions_are_consistent(&self) -> bool {
        if self.questions.is_empty() {
            return false;
        }
        let mut labels = BTreeMap::new();
        let mut attempts = BTreeSet::new();
        let mut questions_by_attempt: BTreeMap<u32, BTreeSet<&str>> = BTreeMap::new();
        let mut attempt_metadata = BTreeMap::new();
        let mut route: Option<&str> = None;
        self.questions.iter().all(|question| {
            let Some(attempt) = question.attempt else {
                return false;
            };
            let Some(question_route) = question.route.as_deref() else {
                return false;
            };
            if route.is_some_and(|route| route != question_route) {
                return false;
            }
            route = Some(question_route);
            let Some(language) = question.language.as_deref() else {
                return false;
            };
            let Some(cross_lingual) = question.cross_lingual else {
                return false;
            };
            let metadata = (
                question.seed,
                question.repetition,
                question.retry_of,
                question.warm_up,
            );
            let previous = attempt_metadata.insert(attempt, metadata);
            let metadata_matches = previous.is_none_or(|previous| previous == metadata);
            questions_by_attempt
                .entry(attempt)
                .or_default()
                .insert(question.id.as_str());
            labels
                .insert(
                    question.id.as_str(),
                    (question.answerable, language, cross_lingual),
                )
                .is_none_or(|previous| previous == (question.answerable, language, cross_lingual))
                && attempts.insert((question.id.as_str(), attempt))
                && metadata_matches
        }) && questions_by_attempt.values().all(|questions| {
            questions_by_attempt
                .values()
                .next()
                .is_some_and(|first| first == questions)
        })
    }

    /// Refuses summaries that do not match their persisted attempt rows.
    fn v2_derived_fields_are_consistent(&self) -> bool {
        let integrity_violation = self
            .questions
            .iter()
            .any(QuestionResult::is_integrity_violation);
        let metrics = measure_attempts(&self.questions, self.seed);
        let cohorts = measure_cohorts(&self.questions);
        let subgroups = measure_subgroups(&self.questions, self.seed);
        self.degraded_searches == self.questions.iter().filter(|row| row.degraded).count()
            && self.integrity_violation == Some(integrity_violation)
            && metrics_match(&self.metrics, &metrics)
            && self
                .cohorts
                .as_ref()
                .is_some_and(|actual| cohorts_match(actual, &cohorts))
            && self
                .subgroups
                .as_ref()
                .is_some_and(|actual| subgroups_match(actual, &subgroups))
    }
}

impl QuestionResult {
    /// Whether no v2-only fields are present.
    fn is_v1(&self) -> bool {
        self.attempt.is_none()
            && self.repetition.is_none()
            && self.retry_of.is_none()
            && self.seed.is_none()
            && self.route.is_none()
            && self.status.is_none()
            && self.elapsed_us.is_none()
            && self.cohort.is_none()
            && self.warm_up.is_none()
            && self.language.is_none()
            && self.cross_lingual.is_none()
    }

    /// Whether every v2-only item field is present and internally valid.
    fn is_v2(&self) -> bool {
        valid_attempt_metadata(
            self.attempt,
            self.repetition,
            self.retry_of,
            self.route.as_deref(),
            self.warm_up,
        ) && self.seed.is_some()
            && self.status.as_ref().is_some_and(|status| match status {
                ItemStatus::Succeeded => true,
                ItemStatus::Failed { kind, reason } => {
                    !kind.trim().is_empty() && !reason.trim().is_empty()
                }
            })
            && self.elapsed_us.is_some_and(|elapsed| {
                self.latency_us == u32::try_from(elapsed).unwrap_or(u32::MAX)
            })
            && self.cohort.is_some()
            && self
                .language
                .as_ref()
                .is_some_and(|language| matches!(language.as_str(), "fr" | "en"))
            && self.cross_lingual.is_some()
    }

    /// Whether the failed row records a collection/generation integrity break.
    pub(in crate::eval) fn is_integrity_violation(&self) -> bool {
        self.status.as_ref().is_some_and(|status| {
            matches!(
                status,
                ItemStatus::Failed { kind, .. }
                    if matches!(kind.as_str(), "wrong_collection" | "wrong_generation")
            )
        })
    }
}

/// The strict frozen report fields required by both reader and writer.
#[expect(
    clippy::too_many_arguments,
    reason = "the shared validator checks every frozen header field"
)]
pub(in crate::eval) fn valid_v2_header(
    suite: &str,
    collection: &str,
    profiles: &BTreeMap<String, String>,
    run_id: Option<&str>,
    candidate_id: Option<&str>,
    card_id: Option<&str>,
    card_digest: Option<&Digest>,
    manifest_digest: Option<&Digest>,
    corpus_digest: Option<&Digest>,
    input_digest: Option<&Digest>,
    mode: Option<TrialMode>,
    planned_repetitions: Option<u32>,
    planned_warm_ups: Option<u32>,
) -> bool {
    !suite.trim().is_empty()
        && !collection.trim().is_empty()
        && profiles
            .iter()
            .all(|(name, value)| !name.trim().is_empty() && !value.trim().is_empty())
        && run_id.is_some_and(|value| !value.trim().is_empty())
        && candidate_id.is_some_and(|value| !value.trim().is_empty())
        && card_id.is_some_and(|value| !value.trim().is_empty())
        && card_digest.is_some()
        && manifest_digest.is_some()
        && corpus_digest.is_some()
        && input_digest.is_some()
        && mode.is_some()
        && planned_repetitions.is_some_and(|count| count > 0)
        && planned_warm_ups.is_some()
}

/// Attempt-identity rules shared by the writer and item reader.
pub(in crate::eval) fn valid_attempt_metadata(
    attempt: Option<u32>,
    repetition: Option<u32>,
    retry_of: Option<u32>,
    route: Option<&str>,
    warm_up: Option<bool>,
) -> bool {
    attempt.is_some_and(|value| value > 0)
        && repetition.is_some_and(|repetition| {
            warm_up == Some(true) && repetition == 0 || warm_up == Some(false) && repetition > 0
        })
        && route.is_some_and(|value| !value.trim().is_empty())
        && retry_of.is_none_or(|original| {
            warm_up == Some(false) && original > 0 && attempt.is_some_and(|value| original != value)
        })
}

/// Exact equality of the deterministic IEEE-754 metric fields.
fn metrics_match(actual: &Metrics, expected: &Metrics) -> bool {
    estimate_match(actual.recall_at_5, expected.recall_at_5)
        && estimate_match(actual.recall_at_10, expected.recall_at_10)
        && estimate_match(actual.mrr_at_10, expected.mrr_at_10)
        && estimate_match(actual.ndcg_at_10, expected.ndcg_at_10)
        && estimate_match(actual.no_answer_accuracy, expected.no_answer_accuracy)
        && estimate_match(actual.false_abstentions, expected.false_abstentions)
        && estimate_match(actual.latency_p50_us, expected.latency_p50_us)
        && estimate_match(actual.latency_p95_us, expected.latency_p95_us)
}

/// Exact equality of optional estimate bits.
fn estimate_match(actual: Option<Estimate>, expected: Option<Estimate>) -> bool {
    match (actual, expected) {
        (Some(actual), Some(expected)) => {
            actual.value.to_bits() == expected.value.to_bits()
                && actual.low.to_bits() == expected.low.to_bits()
                && actual.high.to_bits() == expected.high.to_bits()
        }
        (None, None) => true,
        _ => false,
    }
}

/// Exact equality of the cohort rows, including throughput bits.
fn cohorts_match(
    actual: &BTreeMap<MeasurementCohort, CohortStatistics>,
    expected: &BTreeMap<MeasurementCohort, CohortStatistics>,
) -> bool {
    actual.len() == expected.len()
        && actual.iter().all(|(cohort, actual)| {
            expected.get(cohort).is_some_and(|expected| {
                actual.attempts == expected.attempts
                    && actual.warm_up_attempts == expected.warm_up_attempts
                    && actual.succeeded == expected.succeeded
                    && actual.failed == expected.failed
                    && actual.timed_samples == expected.timed_samples
                    && actual.total_elapsed_us == expected.total_elapsed_us
                    && actual.p50_us == expected.p50_us
                    && actual.p95_us == expected.p95_us
                    && optional_float_match(
                        actual.successful_per_second,
                        expected.successful_per_second,
                    )
            })
        })
}

/// Exact equality of the subgroup rows and their deterministic metrics.
fn subgroups_match(
    actual: &BTreeMap<Subgroup, SubgroupStatistics>,
    expected: &BTreeMap<Subgroup, SubgroupStatistics>,
) -> bool {
    actual.len() == expected.len()
        && actual.iter().all(|(subgroup, actual)| {
            expected.get(subgroup).is_some_and(|expected| {
                actual.question_count == expected.question_count
                    && actual.answerable_questions == expected.answerable_questions
                    && actual.unanswerable_questions == expected.unanswerable_questions
                    && actual.attempt_count == expected.attempt_count
                    && actual.warm_up_attempts == expected.warm_up_attempts
                    && actual.failed_attempts == expected.failed_attempts
                    && actual.ranked_attempts == expected.ranked_attempts
                    && metrics_match(&actual.metrics, &expected.metrics)
            })
        })
}

/// Exact equality of an optional float's bits.
fn optional_float_match(actual: Option<f64>, expected: Option<f64>) -> bool {
    match (actual, expected) {
        (Some(actual), Some(expected)) => actual.to_bits() == expected.to_bits(),
        (None, None) => true,
        _ => false,
    }
}

/// Refuses a version-inconsistent report shape.
fn invalid(message: &'static str) -> serde_json::Error {
    <serde_json::Error as SerdeDeError>::custom(message)
}
