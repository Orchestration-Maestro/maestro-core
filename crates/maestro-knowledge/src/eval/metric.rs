//! The metrics of a run: each a value over a sample of its questions, drawn
//! again and again for its interval. nDCG counts each expected group as one
//! answer item at its best rank; ungrouped names remain separate items.

use super::{
    bootstrap::{estimates, percentile},
    reports::{
        CohortStatistics, Estimate, ItemStatus, MeasurementCohort, Metrics, QuestionResult,
        Subgroup, SubgroupStatistics,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::LOG10_2,
};

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
        let sample: Vec<&QuestionResult> = sample
            .iter()
            .copied()
            .filter(|result| !result.warm_up.unwrap_or(false))
            .collect();
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
            Self::LatencyP50 => latency(&sample, 500),
            Self::LatencyP95 => latency(&sample, 950),
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

/// The metrics of repeated attempts, with bootstrap resamples drawing a
/// question cluster and retaining every one of its attempts together.
pub(super) fn measure_attempts(results: &[QuestionResult], seed: u64) -> Metrics {
    let mut grouped: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, result) in results.iter().enumerate() {
        grouped.entry(&result.id).or_default().push(index);
    }
    let clusters: Vec<Vec<usize>> = grouped
        .into_values()
        .filter(|rows| {
            rows.iter().any(|&index| {
                results
                    .get(index)
                    .is_some_and(|row| !row.warm_up.unwrap_or(false))
            })
        })
        .collect();
    let answerable: Vec<bool> = clusters
        .iter()
        .filter_map(|rows| results.get(*rows.first()?).map(|result| result.answerable))
        .collect();
    let found = estimates(&answerable, seed, |sample| {
        let rows: Vec<&QuestionResult> = sample
            .iter()
            .filter_map(|&cluster| clusters.get(cluster))
            .flatten()
            .filter_map(|&index| results.get(index))
            .collect();
        values(&rows)
    });
    metrics(&found)
}

/// The observed attempt costs grouped by their explicit latency cohort.
pub(super) fn measure_subgroups(
    results: &[QuestionResult],
    seed: u64,
) -> BTreeMap<Subgroup, SubgroupStatistics> {
    [Subgroup::Fr, Subgroup::En, Subgroup::CrossLingual]
        .into_iter()
        .map(|subgroup| {
            let rows: Vec<QuestionResult> = results
                .iter()
                .filter(|row| match subgroup {
                    Subgroup::Fr => row.language.as_deref() == Some("fr"),
                    Subgroup::En => row.language.as_deref() == Some("en"),
                    Subgroup::CrossLingual => row.cross_lingual == Some(true),
                })
                .cloned()
                .collect();
            let mut questions = BTreeSet::new();
            let mut answerable = BTreeSet::new();
            let mut unanswerable = BTreeSet::new();
            for row in &rows {
                questions.insert(row.id.as_str());
                if row.answerable {
                    answerable.insert(row.id.as_str());
                } else {
                    unanswerable.insert(row.id.as_str());
                }
            }
            let warm_up_attempts = rows.iter().filter(|row| row.warm_up == Some(true)).count();
            let failed_attempts = rows
                .iter()
                .filter(|row| matches!(row.status, Some(ItemStatus::Failed { .. })))
                .count();
            let ranked_attempts = rows.len() - warm_up_attempts;
            (
                subgroup,
                SubgroupStatistics {
                    question_count: questions.len(),
                    answerable_questions: answerable.len(),
                    unanswerable_questions: unanswerable.len(),
                    attempt_count: rows.len(),
                    warm_up_attempts,
                    failed_attempts,
                    ranked_attempts,
                    metrics: measure_attempts(&rows, seed),
                },
            )
        })
        .collect()
}

/// Calculates latency cohort counts, percentiles and successful throughput.
pub(super) fn measure_cohorts(
    results: &[QuestionResult],
) -> BTreeMap<MeasurementCohort, CohortStatistics> {
    let mut grouped: BTreeMap<MeasurementCohort, Vec<&QuestionResult>> = BTreeMap::new();
    for result in results {
        grouped
            .entry(result.measurement_cohort())
            .or_default()
            .push(result);
    }
    grouped
        .into_iter()
        .map(|(cohort, rows)| {
            let eligible: Vec<&QuestionResult> = rows
                .iter()
                .copied()
                .filter(|row| {
                    !row.warm_up.unwrap_or(false)
                        && cohort != MeasurementCohort::Unavailable
                        && !row.degraded
                        && row.status == Some(ItemStatus::Succeeded)
                        && row.elapsed_us.is_some()
                })
                .collect();
            let mut sorted: Vec<u64> = eligible.iter().filter_map(|row| row.elapsed_us).collect();
            sorted.sort_unstable();
            let succeeded = rows
                .iter()
                .filter(|row| row.status == Some(ItemStatus::Succeeded))
                .count();
            let failed = rows
                .iter()
                .filter(|row| matches!(row.status, Some(ItemStatus::Failed { .. })))
                .count();
            let timed_successes = eligible.len();
            let total_elapsed_us = rows
                .iter()
                .filter_map(|row| row.elapsed_us)
                .fold(0_u64, u64::saturating_add);
            let timed_elapsed_us = eligible
                .iter()
                .filter_map(|row| row.elapsed_us)
                .fold(0_u64, u64::saturating_add);
            let successful_per_second = (timed_elapsed_us > 0).then(|| {
                usize_as_f64(timed_successes) * 1_000_000.0 / unsigned_as_f64(timed_elapsed_us)
            });
            (
                cohort,
                CohortStatistics {
                    attempts: rows.len(),
                    warm_up_attempts: rows.iter().filter(|row| row.warm_up == Some(true)).count(),
                    succeeded,
                    failed,
                    timed_samples: sorted.len(),
                    total_elapsed_us,
                    p50_us: percentile(&sorted, 500),
                    p95_us: percentile(&sorted, 950),
                    successful_per_second,
                },
            )
        })
        .collect()
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
/// each group gains 1 at the best member rank, while each ungrouped name is
/// its own item; the ideal ranking puts every item first, retrieved or not.
/// 0 for a question that expects none.
fn ndcg(result: &QuestionResult) -> f64 {
    let mut grouped = BTreeMap::new();
    let mut ungrouped = Vec::new();
    for expected in &result.expected {
        if let Some(group) = expected.group.as_deref() {
            let best = grouped.entry(group).or_insert(expected.rank);
            if let Some(rank) = expected.rank {
                *best = Some((*best).map_or(rank, |best| best.min(rank)));
            }
        } else {
            ungrouped.push(expected.rank);
        }
    }
    let item_count = grouped.len() + ungrouped.len();
    let gained: f64 = ungrouped
        .into_iter()
        .chain(grouped.into_values())
        .flatten()
        .map(|rank| discount(rank.get()))
        .sum();
    let ideal: f64 = DISCOUNTS.iter().take(item_count).sum();
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
    let succeeded = !matches!(result.status, Some(ItemStatus::Failed { .. }));
    f64::from(u8::from(succeeded && result.passages == 0))
}

/// Converts a row count to the floating-point representation used for rates.
fn usize_as_f64(value: usize) -> f64 {
    u64::try_from(value).map_or(f64::MAX, unsigned_as_f64)
}

/// Converts an integer into its nearest IEEE-754 f64 value without a lossy cast.
fn unsigned_as_f64(value: u64) -> f64 {
    let upper = u32::try_from(value >> 32).unwrap_or(u32::MAX);
    let lower = u32::try_from(value & u64::from(u32::MAX)).unwrap_or(u32::MAX);
    f64::from(upper) * 4_294_967_296.0 + f64::from(lower)
}

/// The latency at `per_mille` of the questions of `sample` whose search was
/// not degraded, by nearest rank, if it holds any: a degraded search is
/// counted apart, never in the percentiles (SC-S1-004).
fn latency(sample: &[&QuestionResult], per_mille: usize) -> Option<f64> {
    let mut latencies: Vec<u64> = sample
        .iter()
        .filter(|result| {
            !result.degraded
                && !matches!(result.status, Some(ItemStatus::Failed { .. }))
                && matches!(result.cohort, None | Some(MeasurementCohort::Warm))
        })
        .map(|result| result.elapsed_us.unwrap_or(u64::from(result.latency_us)))
        .collect();
    latencies.sort_unstable();
    percentile(&latencies, per_mille).map(unsigned_as_f64)
}
