//! The intervals: 95 % percentile intervals of 2,000 bootstrap resamples,
//! drawn within the answerable and the unanswerable questions apart, by a
//! `SplitMix64` generator the seed starts, so that the same seed gives the
//! same intervals, bit for bit. Two goldens pin them, as an independent
//! reference of the generator, the resamples and the metrics computed them.

use super::support::{Hit, bundle, hit, question, report_of, sections};
use crate::eval::{
    Estimate, Metrics, QuestionResult,
    bootstrap::{Generator, estimates, percentile, resample, strata},
    compare,
    judge::judge,
    metric::{Metric, measure},
};
use std::{cell::Cell, collections::BTreeMap};

/// Five questions: three answerable, found at ranks 1, 4 and never, and two
/// unanswerable, one left empty.
fn results() -> Vec<QuestionResult> {
    let asked = [
        ("found-first", vec!["one"], vec![hit(1, "one", 0.9)]),
        (
            "found-fourth",
            vec!["four"],
            (1..=4)
                .map(|n| hit(n, if n == 4 { "four" } else { "other" }, 1.0 / f64::from(n)))
                .collect(),
        ),
        ("missed", vec!["missed"], vec![hit(1, "other", 0.9)]),
        ("refused", Vec::new(), Vec::new()),
        ("answered", Vec::new(), vec![hit(1, "other", 0.2)]),
    ];
    (100..)
        .step_by(25)
        .zip(asked)
        .map(|(latency, (id, expected, hits))| {
            judge(
                &question(id, !expected.is_empty()),
                &sections(&expected),
                &bundle(&hits),
                latency,
            )
        })
        .collect()
}

/// Sixty questions as a run answers them, and as another run of the same
/// questions does when `lift` is not 0: forty-eight answerable, whose expected
/// section ranks `index % 13 + 1`, `lift` ranks higher but never above the
/// first, and at rank 13 is not retrieved at all, its bundle left empty; and
/// twelve unanswerable, left empty when `index` is a multiple of `2 + lift`.
/// Each is retrieved in `100 + index * (37 - lift) % 1000` µs.
fn many(lift: u32) -> Vec<QuestionResult> {
    (0..60_u32)
        .map(|index| {
            let answerable = index < 48;
            let rank = (index % 13 + 1).saturating_sub(lift).max(1);
            let hits: Vec<Hit> = if answerable && rank < 13 {
                let section = |n| if n == rank { "wanted" } else { "other" };
                (1..=12)
                    .map(|n| hit(n, section(n), 1.0 / f64::from(n)))
                    .collect()
            } else if answerable || index % (2 + lift) == 0 {
                Vec::new()
            } else {
                vec![hit(1, "other", 0.5)]
            };
            let expected: &[&str] = if answerable { &["wanted"] } else { &[] };
            judge(
                &question(&format!("question-{index}"), answerable),
                &sections(expected),
                &bundle(&hits),
                100 + index * (37 - lift) % 1000,
            )
        })
        .collect()
}

/// Each metric's value, low and high end, by name, as `measure(&many(0),
/// 2026)` gives them.
const MEASURED: [(&str, [f64; 3]); 8] = [
    (
        "recall_at_5",
        [
            0.416_666_666_666_666_7,
            0.270_833_333_333_333_3,
            0.541_666_666_666_666_6,
        ],
    ),
    ("recall_at_10", [0.8125, 0.6875, 0.916_666_666_666_666_6]),
    (
        "mrr_at_10",
        [
            0.241_997_354_497_354_4,
            0.171_560_846_560_846_58,
            0.320_370_370_370_370_3,
        ],
    ),
    (
        "ndcg_at_10",
        [
            0.372_607_760_959_072_83,
            0.297_355_047_609_674_8,
            0.443_549_825_715_383,
        ],
    ),
    ("no_answer_accuracy", [0.5, 0.25, 0.75]),
    ("false_abstentions", [0.0625, 0.0, 0.145_833_333_333_333_34]),
    ("latency_p50_us", [544.0, 396.0, 691.0]),
    ("latency_p95_us", [1061.0, 951.0, 1098.0]),
];

/// Each difference's value, low and high end, by name, as comparing
/// `many(2)` with `many(0)` under the seed 2026 gives them.
const COMPARED: [(&str, [f64; 3]); 8] = [
    (
        "recall_at_5",
        [0.166_666_666_666_666_69, 0.0625, 0.270_833_333_333_333_37],
    ),
    (
        "recall_at_10",
        [0.125, 0.041_666_666_666_666_63, 0.229_166_666_666_666_63],
    ),
    (
        "mrr_at_10",
        [
            0.161_747_685_185_185_15,
            0.106_192_129_629_629_57,
            0.221_759_259_259_259_2,
        ],
    ),
    (
        "ndcg_at_10",
        [
            0.153_823_023_490_630_98,
            0.110_558_573_993_195_74,
            0.200_647_948_855_305_57,
        ],
    ),
    ("no_answer_accuracy", [-0.25, -0.5, 0.0]),
    (
        "false_abstentions",
        [-0.0625, -0.145_833_333_333_333_34, 0.0],
    ),
    ("latency_p50_us", [26.0, -54.0, 123.0]),
    ("latency_p95_us", [-16.0, -88.0, 92.0]),
];

/// The bits of `golden`, as [`bits`] gives those of a set of metrics.
fn golden_bits(golden: &[(&'static str, [f64; 3])]) -> Vec<(&'static str, Option<[u64; 3]>)> {
    golden
        .iter()
        .map(|&(name, numbers)| (name, Some(numbers.map(f64::to_bits))))
        .collect()
}

/// Every estimate of `metrics`, by name.
fn estimates_of(metrics: &Metrics) -> Vec<(&'static str, Option<Estimate>)> {
    vec![
        ("recall_at_5", metrics.recall_at_5),
        ("recall_at_10", metrics.recall_at_10),
        ("mrr_at_10", metrics.mrr_at_10),
        ("ndcg_at_10", metrics.ndcg_at_10),
        ("no_answer_accuracy", metrics.no_answer_accuracy),
        ("false_abstentions", metrics.false_abstentions),
        ("latency_p50_us", metrics.latency_p50_us),
        ("latency_p95_us", metrics.latency_p95_us),
    ]
}

/// The bits of every number of `metrics`, so that two sets compare exactly.
fn bits(metrics: &Metrics) -> Vec<(&'static str, Option<[u64; 3]>)> {
    estimates_of(metrics)
        .into_iter()
        .map(|(name, estimate)| {
            let bits = estimate
                .map(|estimate| [estimate.value, estimate.low, estimate.high].map(f64::to_bits));
            (name, bits)
        })
        .collect()
}

#[test]
fn the_generator_is_splitmix64() {
    let mut generator = Generator::new(1_234_567);
    let drawn: Vec<u64> = (0..5).map(|_| generator.next_u64()).collect();
    assert_eq!(
        drawn,
        [
            6_457_827_717_110_365_317,
            3_203_168_211_198_807_973,
            9_817_491_932_198_370_423,
            4_593_380_528_125_082_431,
            16_408_922_859_458_223_821,
        ]
    );
}

#[test]
fn a_draw_below_a_bound_scales_the_generators_output() {
    let mut generator = Generator::new(1_234_567);
    let drawn: Vec<usize> = (0..10).map(|_| generator.below(10)).collect();
    assert_eq!(drawn, [3, 1, 5, 2, 8, 4, 5, 2, 4, 8]);
}

#[test]
fn a_resample_draws_each_stratum_to_its_own_size() {
    let strata = strata([true, false, true, true, false].into_iter());
    assert_eq!(strata, [vec![0, 2, 3], vec![1, 4]]);
    let mut generator = Generator::new(1_234_567);
    assert_eq!(resample(&strata, &mut generator), [2, 0, 2, 1, 4]);
}

#[test]
fn percentiles_are_by_nearest_rank() {
    let latencies = [10, 20, 30, 40];
    assert_eq!(percentile(&latencies, 250), Some(10));
    assert_eq!(percentile(&latencies, 500), Some(20));
    assert_eq!(percentile(&latencies, 501), Some(30));
    assert_eq!(percentile(&latencies, 950), Some(40));
    assert_eq!(percentile(&latencies, 1000), Some(40));
    assert_eq!(percentile::<u32>(&[], 500), None);
    let draws: Vec<u32> = (1..=2000).collect();
    assert_eq!(percentile(&draws, 25), Some(50));
    assert_eq!(percentile(&draws, 975), Some(1950));
}

#[test]
fn an_interval_holds_the_2_5th_and_97_5th_percentiles_of_2000_resamples() {
    // The statistic numbers its calls: 0 for the whole suite, then 1 to 2,000
    // for the resamples, in the order they are drawn.
    let calls = Cell::new(0_u32);
    let found = estimates(&[true, false], 3, |sample| {
        // One answerable and one unanswerable question: each resample draws
        // one of each, in that order.
        assert_eq!(sample, [0, 1]);
        let call = calls.get();
        calls.set(call + 1);
        BTreeMap::from([(Metric::RecallAt5, f64::from(call))])
    });
    assert_eq!(calls.get(), 2001);
    assert_eq!(
        found,
        BTreeMap::from([(
            Metric::RecallAt5,
            Estimate {
                value: 0.0,
                low: 50.0,
                high: 1950.0,
            }
        )])
    );
}

#[test]
fn the_same_seed_gives_the_same_intervals_bit_for_bit() {
    let results = many(0);
    let first = measure(&results, 2026);
    assert_eq!(bits(&first), bits(&measure(&results, 2026)));
    assert_ne!(bits(&first), bits(&measure(&results, 2027)));
    for (name, estimate) in estimates_of(&first) {
        let estimate = estimate.unwrap_or_else(|| panic!("{name} is defined"));
        assert!(estimate.low <= estimate.high, "{name}: {estimate:?}");
    }
}

#[test]
fn a_metric_equal_on_every_question_has_the_interval_of_that_value() {
    let results: Vec<QuestionResult> = results()
        .into_iter()
        .filter(|result| result.id == "found-first")
        .collect();
    let metrics = measure(&results, 1);
    let certain = Estimate {
        value: 1.0,
        low: 1.0,
        high: 1.0,
    };
    assert_eq!(metrics.recall_at_5, Some(certain));
    assert_eq!(metrics.mrr_at_10, Some(certain));
    assert_eq!(metrics.ndcg_at_10, Some(certain));
    let latency = Estimate {
        value: 100.0,
        low: 100.0,
        high: 100.0,
    };
    assert_eq!(metrics.latency_p50_us, Some(latency));
}

#[test]
fn the_intervals_of_a_run_are_those_of_the_reference() {
    assert_eq!(bits(&measure(&many(0), 2026)), golden_bits(&MEASURED));
}

#[test]
fn the_intervals_of_a_comparison_are_those_of_the_reference() {
    let comparison = compare(&report_of(many(0)), &report_of(many(2)), 2026).unwrap();
    assert_eq!(comparison.questions, 60);
    assert_eq!(bits(&comparison.differences), golden_bits(&COMPARED));
}
