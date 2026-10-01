//! V2 report metadata and cohort/subgroup summaries.

use super::metrics::Metrics;
use serde::{Deserialize, Serialize};
/// Whether an item succeeded or failed, retaining the typed failure reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemStatus {
    /// Retrieval returned a valid bundle for the pinned generation.
    Succeeded,
    /// Retrieval failed; the item scores zero and remains in the report.
    Failed {
        /// Stable failure category.
        kind: String,
        /// Caller-visible failure detail.
        reason: String,
    },
}

/// Whether latency was observed under a warm, cold, unavailable, or unknown
/// runtime/cache state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementCohort {
    /// Residency and cache provenance establish a warm request.
    Warm,
    /// Model load or cold cache was observed.
    Cold,
    /// The route did not provide a latency sample.
    Unavailable,
    /// Evidence cannot establish warmth or coldness, including all v1 imports.
    Unknown,
}

/// Latency and cost accounting for one measured cohort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CohortStatistics {
    /// Every attempted row, including warm-ups and failures.
    pub attempts: usize,
    /// Attempt rows deliberately excluded from ranked samples.
    pub warm_up_attempts: usize,
    /// Successful attempt rows.
    pub succeeded: usize,
    /// Failed attempt rows.
    pub failed: usize,
    /// Non-warm-up timed rows included in p50/p95.
    pub timed_samples: usize,
    /// Sum of every observed row's elapsed time, including warm-ups/failures.
    pub total_elapsed_us: u64,
    /// Nearest-rank median of timed rows, in microseconds.
    pub p50_us: Option<u64>,
    /// Nearest-rank 95th percentile of timed rows, in microseconds.
    pub p95_us: Option<u64>,
    /// Successful non-warm-up rows per second over timed-row elapsed time.
    pub successful_per_second: Option<f64>,
}

/// The fixed subgroup in the manifest/report protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subgroup {
    /// French-language questions.
    Fr,
    /// English-language questions.
    En,
    /// Questions explicitly labelled cross-lingual.
    CrossLingual,
}

/// Question/attempt counts and clustered metrics for a frozen subgroup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubgroupStatistics {
    /// Distinct question IDs in this subgroup.
    pub question_count: usize,
    /// Distinct answerable question IDs.
    pub answerable_questions: usize,
    /// Distinct unanswerable question IDs.
    pub unanswerable_questions: usize,
    /// All attempt rows, including failures and warm-ups.
    pub attempt_count: usize,
    /// Attempt rows excluded from ranked samples.
    pub warm_up_attempts: usize,
    /// Failed attempt rows.
    pub failed_attempts: usize,
    /// Non-warm-up attempt rows in the quality-metric denominator.
    pub ranked_attempts: usize,
    /// Clustered metrics; empty subgroup estimates remain unavailable.
    pub metrics: Metrics,
}

/// Evaluation provenance attached to the v2 report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialMode {
    /// Fake models and synthetic fixtures; never selection proof.
    Synthetic,
    /// Authorized real evaluation.
    Real,
}
