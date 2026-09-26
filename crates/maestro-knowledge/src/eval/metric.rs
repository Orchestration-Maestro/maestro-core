//! The metrics of a run: each a value over a sample of its questions, drawn
//! again and again for its interval.

use super::{
    bootstrap::{estimates, percentile},
    report::{Estimate, Metrics, QuestionResult},
};
use std::{collections::BTreeMap, f64::consts::LOG10_2};

/// The last rank MRR@10 and nDCG@10 read.
const DEPTH: u32 = 10;

/// The gain of a relevant passage at each rank from 1 to [`DEPTH`],
/// 1 / log2(rank + 1), each the `f64` nearest to it. They are written out
/// rather than computed, since the platform's math library gives `f64::log2`
/// its precision, so that nDCG@10 has the same bits on every platform.
pub(super) const DISCOUNTS: [f64; 10] = [
    1.0,
    0.630_929_753_571_457_4,
    0.5,
    0.430_676_558_073_393_06,
    0.386_852_807_234_541_6,
    0.356_207_187_108_022_2,
    1.0 / 3.0,
    0.315_464_876_785_728_7,
    // 1 / log2(10) is log10(2).
    LOG10_2,
    0.289_064_826_317_887_9,
];

/// A metric of a run, as [`Metrics`] names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Metric {
    /// [`Metrics::recall_at_5`].
    RecallAt5,
    /// [`Metrics::recall_at_10`].
    RecallAt10,
    /// [`Metrics::mrr_at_10`].
    MrrAt10,
    /// [`Metrics::ndcg_at_10`].
    NdcgAt10,
    /// [`Metrics::no_answer_accuracy`].
    NoAnswerAccuracy,
    /// [`Metrics::false_abstentions`].
    FalseAbstentions,
    /// [`Metrics::latency_p50_us`].
    LatencyP50,
    /// [`Metrics::latency_p95_us`].
    LatencyP95,
}

impl Metric {
    /// Every metric.
    const ALL: [Self; 8] = [
        Self::RecallAt5,
        Self::RecallAt10,
        Self::MrrAt10,
        Self::NdcgAt10,
        Self::NoAnswerAccuracy,
        Self::FalseAbstentions,
        Self::LatencyP50,
        Self::LatencyP95,
    ];

    /// Its value over `sample`, whose questions may repeat, if it covers any
    /// of them: the answerable ones for the ranking metrics and the false
    /// abstentions, the unanswerable ones for the no-answer accuracy, and
    /// those whose search was not degraded for the latencies.
    fn value(self, sample: &[&QuestionResult]) -> Option<f64> {
        let answerable = sample.iter().filter(|result| result.answerable);
        match self {
            Self::RecallAt5 => mean(answerable.map(|result| hit(result, 5))),
            Self::RecallAt10 => mean(answerable.map(|result| hit(result, DEPTH))),
            Self::MrrAt10 => mean(answerable.map(|result| reciprocal_rank(result))),
            Self::NdcgAt10 => mean(answerable.map(|result| ndcg(result))),
            Self::NoAnswerAccuracy => mean(
                sample
                    .iter()
                    .filter(|result| !result.answerable)
                    .map(|result| abstained(result)),
            ),
            Self::FalseAbstentions => mean(answerable.map(|result| abstained(result))),
            Self::LatencyP50 => latency(sample, 500),
            Self::LatencyP95 => latency(sample, 950),
        }
    }
}

/// The metrics of `results`, each with its interval, whose resamples are
/// drawn with `seed`.
pub(super) fn measure(results: &[QuestionResult], seed: u64) -> Metrics {
    let answerable: Vec<bool> = results.iter().map(|result| result.answerable).collect();
    let found = estimates(&answerable, seed, |sample| {
        let sample: Vec<&QuestionResult> = sample
            .iter()
            .filter_map(|&index| results.get(index))
            .collect();
        values(&sample)
    });
    metrics(&found)
}

/// Every metric `sample` covers, with its value.
pub(super) fn values(sample: &[&QuestionResult]) -> BTreeMap<Metric, f64> {
    Metric::ALL
        .into_iter()
        .filter_map(|metric| Some((metric, metric.value(sample)?)))
        .collect()
}

/// The metrics `found` estimates, each absent when it is.
pub(super) fn metrics(found: &BTreeMap<Metric, Estimate>) -> Metrics {
    let estimate = |metric| found.get(&metric).copied();
    Metrics {
        recall_at_5: estimate(Metric::RecallAt5),
        recall_at_10: estimate(Metric::RecallAt10),
        mrr_at_10: estimate(Metric::MrrAt10),
        ndcg_at_10: estimate(Metric::NdcgAt10),
        no_answer_accuracy: estimate(Metric::NoAnswerAccuracy),
        false_abstentions: estimate(Metric::FalseAbstentions),
        latency_p50_us: estimate(Metric::LatencyP50),
        latency_p95_us: estimate(Metric::LatencyP95),
    }
}

/// The mean of `values`, if there is any.
fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, count) = values.fold((0.0, 0.0), |(sum, count), value| (sum + value, count + 1.0));
    (count > 0.0).then(|| sum / count)
}

/// 1 when an expected section of `result` ranks within `cutoff`, else 0.
fn hit(result: &QuestionResult, cutoff: u32) -> f64 {
    let within = result.best_rank().is_some_and(|rank| rank <= cutoff);
    f64::from(u8::from(within))
}

/// The reciprocal of the best rank of an expected section of `result`, or 0
/// when none ranks within [`DEPTH`].
fn reciprocal_rank(result: &QuestionResult) -> f64 {
    result
        .best_rank()
        .filter(|&rank| rank <= DEPTH)
        .map_or(0.0, |rank| f64::from(rank).recip())
}

/// The normalized discounted cumulative gain of `result` within [`DEPTH`]:
/// each expected section gains 1 at its best rank, discounted by the
/// logarithm of that rank plus one, over the gain of the ideal ranking, with
/// every expected section first, retrieved or not; 0 for a question that
/// expects none.
fn ndcg(result: &QuestionResult) -> f64 {
    let gained: f64 = result
        .expected
        .iter()
        .filter_map(|expected| expected.rank)
        .map(discount)
        .sum();
    let ideal: f64 = DISCOUNTS.iter().take(result.expected.len()).sum();
    if ideal > 0.0 { gained / ideal } else { 0.0 }
}

/// The gain of a relevant passage at `rank`, from 1, as [`DISCOUNTS`] gives
/// it; none below the top [`DEPTH`].
fn discount(rank: u32) -> f64 {
    let index = rank
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok());
    index
        .and_then(|index| DISCOUNTS.get(index))
        .copied()
        .unwrap_or(0.0)
}

/// 1 when the bundle of `result` holds no passage, else 0.
fn abstained(result: &QuestionResult) -> f64 {
    f64::from(u8::from(result.passages == 0))
}

/// The latency at `per_mille` of the questions of `sample` whose search was
/// not degraded, by nearest rank, if it holds any: a degraded search is
/// counted apart, never in the percentiles (SC-S1-004).
fn latency(sample: &[&QuestionResult], per_mille: usize) -> Option<f64> {
    let mut latencies: Vec<u32> = sample
        .iter()
        .filter(|result| !result.degraded)
        .map(|result| result.latency_us)
        .collect();
    latencies.sort_unstable();
    percentile(&latencies, per_mille).map(f64::from)
}
