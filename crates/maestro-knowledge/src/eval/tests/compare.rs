//! `compare` pairs two runs by question and gives, for each metric, the
//! candidate's value minus the baseline's, with a 95 % interval of 2,000
//! paired bootstrap resamples under the seed it is given.

use super::support::{Hit, bundle, hit, question, report_of, sections};
use crate::eval::{
    CompareError, Comparison, Estimate, QuestionResult, Report, compare, judge::judge,
};

/// A question as a run answered it: its id, the section it expects if any,
/// its hits and its latency.
type Answered = (&'static str, Option<&'static str>, Vec<Hit>, u32);

/// A report of `answered`, in that order.
fn report(answered: Vec<Answered>) -> Report {
    let questions: Vec<QuestionResult> = answered
        .into_iter()
        .map(|(id, expected, hits, latency)| {
            let expected: Vec<&str> = expected.into_iter().collect();
            judge(
                &question(id, !expected.is_empty()),
                &sections(&expected),
                &bundle(&hits),
                latency,
            )
        })
        .collect();
    report_of(questions)
}

/// The baseline: `found` at rank 1, `lost` never found, `refused` left empty.
fn baseline() -> Report {
    report(vec![
        ("found", Some("found"), vec![hit(1, "found", 0.9)], 100),
        ("lost", Some("lost"), vec![hit(1, "other", 0.9)], 200),
        ("refused", None, Vec::new(), 300),
    ])
}

/// The candidate: `found` at rank 1, `lost` at rank 2, `refused` answered,
/// each 50 µs slower.
fn candidate() -> Report {
    report(vec![
        ("found", Some("found"), vec![hit(1, "found", 0.9)], 150),
        (
            "lost",
            Some("lost"),
            vec![hit(1, "other", 0.9), hit(2, "lost", 0.5)],
            250,
        ),
        ("refused", None, vec![hit(1, "other", 0.3)], 350),
    ])
}

/// The estimate `value` in `[low, high]`.
fn estimate(value: f64, low: f64, high: f64) -> Estimate {
    Estimate { value, low, high }
}

#[test]
fn compare_gives_the_candidate_minus_the_baseline_with_paired_intervals() {
    let comparison = compare(&baseline(), &candidate(), 2026).unwrap();
    assert_eq!(comparison.seed, 2026);
    assert_eq!(comparison.questions, 3);
    let differences = comparison.differences;
    // A resample draws `lost` twice, once or never among the two answerable
    // questions: a gain of 1, 0.5 or 0 in recall, 0.5, 0.25 or 0 in MRR.
    assert_eq!(differences.recall_at_5, Some(estimate(0.5, 0.0, 1.0)));
    assert_eq!(differences.recall_at_10, Some(estimate(0.5, 0.0, 1.0)));
    assert_eq!(differences.mrr_at_10, Some(estimate(0.25, 0.0, 0.5)));
    assert_eq!(
        differences.no_answer_accuracy,
        Some(estimate(-1.0, -1.0, -1.0))
    );
    assert_eq!(differences.false_abstentions, Some(estimate(0.0, 0.0, 0.0)));
    assert_eq!(differences.latency_p50_us, Some(estimate(50.0, 50.0, 50.0)));
    assert_eq!(differences.latency_p95_us, Some(estimate(50.0, 50.0, 50.0)));
}

#[test]
fn the_same_seed_gives_the_same_comparison_bit_for_bit() {
    let first = compare(&baseline(), &candidate(), 9).unwrap();
    let again = compare(&baseline(), &candidate(), 9).unwrap();
    let bits = |comparison: &Comparison| {
        let differences = &comparison.differences;
        [
            differences.recall_at_5,
            differences.recall_at_10,
            differences.mrr_at_10,
            differences.ndcg_at_10,
            differences.no_answer_accuracy,
            differences.false_abstentions,
            differences.latency_p50_us,
            differences.latency_p95_us,
        ]
        .map(|estimate| estimate.map(|estimate| [estimate.low, estimate.high].map(f64::to_bits)))
    };
    assert_eq!(bits(&first), bits(&again));
}

#[test]
fn questions_pair_by_id_whatever_their_order() {
    let mut reversed = candidate();
    reversed.questions.reverse();
    assert_eq!(
        compare(&baseline(), &reversed, 4).unwrap(),
        compare(&baseline(), &candidate(), 4).unwrap()
    );
}

#[test]
fn a_question_one_run_lacks_is_refused() {
    let mut shorter = candidate();
    shorter.questions.pop();
    let error = compare(&baseline(), &shorter, 1).unwrap_err();
    assert!(matches!(&error, CompareError::Unpaired { id } if id == "refused"));
    assert_eq!(
        error.to_string(),
        "the question refused is in one run only, so the runs cannot be paired"
    );
    let error = compare(&shorter, &baseline(), 1).unwrap_err();
    assert!(matches!(&error, CompareError::Unpaired { id } if id == "refused"));
}

#[test]
fn a_question_given_twice_is_refused() {
    let mut repeated = candidate();
    repeated.questions.push(repeated.questions[0].clone());
    let error = compare(&baseline(), &repeated, 1).unwrap_err();
    assert!(matches!(&error, CompareError::Repeated { id } if id == "found"));
    assert_eq!(
        error.to_string(),
        "the question found is given twice in one run, so the runs cannot be paired"
    );
    let error = compare(&repeated, &baseline(), 1).unwrap_err();
    assert!(matches!(&error, CompareError::Repeated { id } if id == "found"));
}

#[test]
fn a_question_answerable_in_one_run_only_is_refused() {
    let mut changed = candidate();
    changed.questions[2].answerable = true;
    let error = compare(&baseline(), &changed, 1).unwrap_err();
    assert!(matches!(&error, CompareError::Answerability { id } if id == "refused"));
    assert_eq!(
        error.to_string(),
        "the question refused is answerable in one run only, so the runs cannot be paired"
    );
}

#[test]
fn a_report_of_another_suite_is_refused() {
    let mut other = candidate();
    other.suite = "other".to_owned();
    let error = compare(&baseline(), &other, 1).unwrap_err();
    assert_eq!(
        error,
        CompareError::Suite {
            baseline: "synthetic".to_owned(),
            candidate: "other".to_owned(),
        }
    );
    assert_eq!(
        error.to_string(),
        "the baseline ran the suite synthetic and the candidate the suite other, \
         so the runs cannot be paired"
    );
}
