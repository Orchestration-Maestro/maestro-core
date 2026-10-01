use super::{
    support::bundle,
    v2::{header_v2, suite},
};
use crate::eval::{
    HeaderV2, QuestionResult, Report, Schema, merge_v2_attempts,
    metric::{measure_attempts, measure_cohorts, measure_subgroups},
    run_v2,
};
fn report() -> Report {
    attempt(header_v2(), 1, None, false, false)
}

fn attempt(
    mut header: HeaderV2,
    number: u32,
    retry_of: Option<u32>,
    warm_up: bool,
    fail: bool,
) -> Report {
    header.attempt = number;
    header.retry_of = retry_of;
    header.warm_up = warm_up;
    run_v2(
        header,
        &suite(&["question"]),
        |_| Ok::<_, &str>(None),
        |_| {
            if fail {
                Err("timeout")
            } else {
                Ok(bundle(&[]))
            }
        },
    )
    .unwrap()
}

fn refresh_summaries(report: &mut Report) {
    report.degraded_searches = report.questions.iter().filter(|row| row.degraded).count();
    report.integrity_violation = Some(
        report
            .questions
            .iter()
            .any(QuestionResult::is_integrity_violation),
    );
    report.metrics = measure_attempts(&report.questions, report.seed);
    report.cohorts = Some(measure_cohorts(&report.questions));
    report.subgroups = Some(measure_subgroups(&report.questions, report.seed));
}

#[test]
fn merge_refuses_empty_input_and_invalid_first_reports_precisely() {
    assert_eq!(
        merge_v2_attempts(&[]).unwrap_err().to_string(),
        "at least one v2 attempt report is required"
    );

    let mut empty = report();
    empty.questions.clear();
    let mut wrong_schema = report();
    wrong_schema.schema = Schema::V1;
    for invalid in [empty, wrong_schema] {
        assert_eq!(
            merge_v2_attempts(&[invalid]).unwrap_err().to_string(),
            "only nonempty v2 attempt reports can be merged"
        );
    }
}

#[test]
fn merge_refuses_reports_with_different_question_rows() {
    let first = report();
    let mut other = first.clone();
    other.questions[0].id = "other".to_owned();
    refresh_summaries(&mut other);

    assert_eq!(
        merge_v2_attempts(&[first.clone(), other])
            .unwrap_err()
            .to_string(),
        "attempt reports differ in frozen header, route or suite rows"
    );

    let mut empty = first.clone();
    empty.questions.clear();
    assert_eq!(
        merge_v2_attempts(&[first.clone(), empty])
            .unwrap_err()
            .to_string(),
        "attempt reports differ in frozen header, route or suite rows"
    );

    let mut wrong_schema = first.clone();
    wrong_schema.schema = Schema::V1;
    assert!(merge_v2_attempts(&[first, wrong_schema]).is_err());
}

#[test]
fn merge_requires_retries_to_follow_a_failed_original_scored_attempt() {
    let header = header_v2();
    let successful_original = attempt(header.clone(), 1, None, false, false);
    let retry_of_success = attempt(header.clone(), 2, Some(1), false, false);
    assert!(merge_v2_attempts(&[successful_original, retry_of_success]).is_err());

    let original_after_retry = attempt(header.clone(), 2, None, false, true);
    let retry_before_original = attempt(header.clone(), 1, Some(2), false, false);
    assert!(merge_v2_attempts(&[retry_before_original, original_after_retry]).is_err());

    let original = attempt(header.clone(), 1, None, false, true);
    let retry = attempt(header.clone(), 2, Some(1), false, true);
    let retry_of_retry = attempt(header.clone(), 3, Some(2), false, false);
    assert!(merge_v2_attempts(&[original, retry, retry_of_retry]).is_err());

    let different_repetition_original = attempt(header.clone(), 1, None, false, true);
    let mut different_repetition_header = header;
    different_repetition_header.repetition = 2;
    let different_repetition_retry = attempt(different_repetition_header, 2, Some(1), false, false);
    assert!(
        merge_v2_attempts(&[different_repetition_original, different_repetition_retry]).is_err()
    );
}
