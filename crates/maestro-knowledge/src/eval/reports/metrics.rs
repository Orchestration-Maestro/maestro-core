//! Summary metrics shared by the v1 and v2 report contracts.

use crate::shape;
use serde::{Deserialize, Deserializer, Serialize};

/// The metrics of a run, or the differences of two: each an estimate with
/// its interval, absent when it covers no question.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Metrics {
    /// The share of answerable questions with an expected section in the top
    /// 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub recall_at_5: Option<Estimate>,
    /// The share of answerable questions with an expected section in the top
    /// 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub recall_at_10: Option<Estimate>,
    /// The mean over answerable questions of the reciprocal rank of the
    /// first expected section, 0 below the top 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub mrr_at_10: Option<Estimate>,
    /// The mean over answerable questions of the normalized discounted
    /// cumulative gain in the top 10, each answer item gaining 1: a group is
    /// one item at its best member rank, and every ungrouped name is its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub ndcg_at_10: Option<Estimate>,
    /// The share of unanswerable questions whose bundle holds no passage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub no_answer_accuracy: Option<Estimate>,
    /// The share of answerable questions whose bundle holds no passage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub false_abstentions: Option<Estimate>,
    /// The median latency of a retrieval, in microseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub latency_p50_us: Option<Estimate>,
    /// The 95th percentile of a retrieval's latency, in microseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub latency_p95_us: Option<Estimate>,
}

/// A metric's value over every question, and its 95 % interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Estimate {
    /// The value over every question.
    pub value: f64,
    /// The interval's low end: the 2.5th percentile of the resamples.
    pub low: f64,
    /// The interval's high end: the 97.5th percentile of the resamples.
    pub high: f64,
}

/// An estimate, from a JSON object only: a metric that covers no question is
/// left out, never written `null`.
fn estimate<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Estimate>, D::Error> {
    shape::object(deserializer).map(Some)
}
