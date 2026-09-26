//! Each metric of a run against values computed by hand on a small suite:
//! ties, a question with two expected sections, one found late, one found
//! sixth, an answerable question left empty, and two unanswerable ones.

use super::support::{Hit, bundle, close, hit, question, sections};
use crate::eval::{
    Metrics, QuestionResult,
    judge::judge,
    metric::{DISCOUNTS, measure},
};
use std::num::NonZeroU32;

/// The results of the small suite: its six answerable questions, then its two
/// unanswerable ones, answered in 10, 20, … 80 µs.
fn results() -> Vec<QuestionResult> {
    let late: Vec<Hit> = (1..=12)
        .map(|n| {
            hit(
                n,
                if n == 12 { "late" } else { "other" },
                1.0 / f64::from(n),
            )
        })
        .collect();
    let sixth: Vec<Hit> = (1..=7)
        .map(|n| {
            hit(
                n,
                if n == 6 { "sixth" } else { "other" },
                1.0 / f64::from(n),
            )
        })
        .collect();
    let asked = [
        (
            "first",
            vec!["first"],
            vec![hit(1, "first", 0.9), hit(2, "other", 0.5)],
        ),
        (
            "two",
            vec!["two-a", "two-b"],
            (1..=7)
                .map(|n| {
                    let section = match n {
                        3 => "two-a",
                        7 => "two-b",
                        _ => "other",
                    };
                    hit(n, section, 1.0 / f64::from(n))
                })
                .collect(),
        ),
        (
            "tied",
            vec!["tied"],
            vec![
                hit(3, "tied", 0.5),
                hit(1, "other", 0.9),
                hit(2, "other", 0.5),
            ],
        ),
        ("late", vec!["late"], late),
        ("sixth", vec!["sixth"], sixth),
        ("empty", vec!["empty"], Vec::new()),
        ("none-right", Vec::new(), Vec::new()),
        ("none-wrong", Vec::new(), vec![hit(1, "other", 0.4)]),
    ];
    (10..)
        .step_by(10)
        .zip(asked)
        .map(|(latency, (id, expected, hits))| {
            let answerable = !expected.is_empty();
            judge(
                &question(id, answerable),
                &sections(&expected),
                &bundle(&hits),
                latency,
            )
        })
        .collect()
}

/// The metrics of the small suite, with the seed 7.
fn metrics() -> Metrics {
    measure(&results(), 7)
}

#[test]
fn recall_at_k_is_the_share_of_answerable_questions_with_an_expected_section_in_the_top_k() {
    let metrics = metrics();
    // first, two and tied in the top 5; sixth too in the top 10.
    close(metrics.recall_at_5.unwrap().value, 3.0 / 6.0);
    close(metrics.recall_at_10.unwrap().value, 4.0 / 6.0);
}

#[test]
fn mrr_at_10_is_the_mean_reciprocal_rank_of_the_first_expected_section() {
    // first 1, two 1/3, tied 1/3 once the tie breaks by number, late 0 (rank
    // 12), sixth 1/6 and empty 0.
    close(metrics().mrr_at_10.unwrap().value, 0.305_555_555_555_555_5);
}

#[test]
fn ndcg_at_10_gains_one_for_each_expected_section() {
    // two: (1/log2 4 + 1/log2 8) / (1 + 1/log2 3); tied: 1/log2 4; sixth:
    // 1/log2 7; late and empty: 0.
    close(
        metrics().ndcg_at_10.unwrap().value,
        (1.0 + 0.510_955_993_971_215_3 + 0.5 + 0.356_207_187_108_022_2) / 6.0,
    );
}

#[test]
fn no_answer_accuracy_counts_the_unanswerable_questions_left_empty() {
    close(metrics().no_answer_accuracy.unwrap().value, 0.5);
}

#[test]
fn false_abstentions_count_the_answerable_questions_left_empty() {
    close(metrics().false_abstentions.unwrap().value, 1.0 / 6.0);
}

#[test]
fn latency_percentiles_are_nearest_rank_values_in_microseconds() {
    let metrics = metrics();
    close(metrics.latency_p50_us.unwrap().value, 40.0);
    close(metrics.latency_p95_us.unwrap().value, 80.0);
}

#[test]
fn a_question_scores_its_own_ranks() {
    let results = results();
    let two = results.iter().find(|result| result.id == "two").unwrap();
    assert!(two.answerable);
    assert_eq!(two.passages, 7);
    assert_eq!(two.latency_us, 20);
    let ranks: Vec<_> = two
        .expected
        .iter()
        .map(|expected| expected.rank.map(NonZeroU32::get))
        .collect();
    assert_eq!(ranks, [Some(3), Some(7)]);
    assert_eq!(two.best_rank(), Some(3));
    let wrong = results
        .iter()
        .find(|result| result.id == "none-wrong")
        .unwrap();
    assert!(!wrong.answerable);
    assert_eq!(wrong.passages, 1);
    assert_eq!(wrong.best_rank(), None);
}

#[test]
fn each_cutoff_counts_its_own_rank() {
    let ranked_at = |rank: u32| {
        let hits: Vec<Hit> = (1..=11)
            .map(|n| {
                hit(
                    n,
                    if n == rank { "wanted" } else { "other" },
                    1.0 / f64::from(n),
                )
            })
            .collect();
        let asked = question(&format!("ranked-{rank}"), true);
        judge(&asked, &sections(&["wanted"]), &bundle(&hits), 1)
    };
    let metrics = measure(&[ranked_at(5), ranked_at(10), ranked_at(11)], 1);
    close(metrics.recall_at_5.unwrap().value, 1.0 / 3.0);
    close(metrics.recall_at_10.unwrap().value, 2.0 / 3.0);
    close(
        metrics.mrr_at_10.unwrap().value,
        (1.0 / 5.0 + 1.0 / 10.0) / 3.0,
    );
    // 1/log2 6 and 1/log2 11; rank 11 gains nothing.
    close(
        metrics.ndcg_at_10.unwrap().value,
        (0.386_852_807_234_541_63 + 0.289_064_826_317_887_9) / 3.0,
    );
}

#[test]
fn the_ideal_ranking_holds_every_expected_section_even_one_not_retrieved() {
    // Of the two sections, only `found` is retrieved, first: 1 / (1 + 1/log2 3).
    // An ideal ranking of the retrieved sections alone would give 1.
    let half = judge(
        &question("half", true),
        &sections(&["found", "lost"]),
        &bundle(&[hit(1, "found", 0.9), hit(2, "other", 0.5)]),
        1,
    );
    close(
        measure(&[half], 1).ndcg_at_10.unwrap().value,
        0.613_147_192_765_458_4,
    );
}

#[test]
fn each_discount_is_one_over_log2_of_its_rank_plus_one() {
    assert_eq!(DISCOUNTS.len(), 10);
    for (rank, discount) in (1_u32..).zip(DISCOUNTS) {
        let computed = f64::from(rank + 1).log2().recip();
        assert!(
            (discount - computed).abs() <= 1e-15,
            "rank {rank}: {discount}, where log2 gives {computed}"
        );
    }
}

#[test]
fn an_answerable_question_without_an_expected_section_gains_nothing() {
    // A report read back may hold one, although no suite does.
    let odd = judge(
        &question("odd", true),
        &[],
        &bundle(&[hit(1, "other", 0.5)]),
        1,
    );
    let metrics = measure(&[odd], 1);
    close(metrics.ndcg_at_10.unwrap().value, 0.0);
    close(metrics.mrr_at_10.unwrap().value, 0.0);
}

#[test]
fn metrics_that_cover_no_question_are_absent() {
    let unanswerable: Vec<QuestionResult> = results()
        .into_iter()
        .filter(|result| !result.answerable)
        .collect();
    let metrics = measure(&unanswerable, 7);
    assert_eq!(metrics.recall_at_5, None);
    assert_eq!(metrics.recall_at_10, None);
    assert_eq!(metrics.mrr_at_10, None);
    assert_eq!(metrics.ndcg_at_10, None);
    assert_eq!(metrics.false_abstentions, None);
    close(metrics.no_answer_accuracy.unwrap().value, 0.5);
    let answerable: Vec<QuestionResult> = results()
        .into_iter()
        .filter(|result| result.answerable)
        .collect();
    assert_eq!(measure(&answerable, 7).no_answer_accuracy, None);
}
