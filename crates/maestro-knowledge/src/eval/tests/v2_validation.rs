use super::{
    support::{bundle, report_of},
    v2::{header_v2, suite},
};
use crate::eval::{
    ItemStatus, MeasurementCohort, QuestionResult, Report,
    metric::{measure_attempts, measure_cohorts, measure_subgroups},
    run_v2,
};
use serde_json::{Value, json};
use std::str::FromStr;

fn report() -> Report {
    run_v2(
        header_v2(),
        &suite(&["question"]),
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
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

fn refuses(mut report: Report) -> bool {
    refresh_summaries(&mut report);
    Report::from_str(&serde_json::to_string(&report).unwrap()).is_err()
}

#[test]
fn v1_reports_refuse_each_independent_v2_header_field() {
    let source = serde_json::to_value(report()).unwrap();
    let fields = [
        "run_id",
        "candidate_id",
        "card_id",
        "card_digest",
        "manifest_digest",
        "planned_repetitions",
        "planned_warm_ups",
        "corpus_digest",
        "input_digest",
        "mode",
        "integrity_violation",
        "cohorts",
        "subgroups",
    ];
    for field in fields {
        let mut value = serde_json::to_value(report_of(Vec::new())).unwrap();
        value[field] = source[field].clone();
        assert!(Report::from_str(&value.to_string()).is_err(), "{field}");
    }
}

#[test]
fn v1_reports_refuse_each_independent_v2_question_field() {
    let source = serde_json::to_value(report()).unwrap();
    let fields = [
        ("attempt", json!(1)),
        ("repetition", json!(1)),
        ("retry_of", json!(1)),
        ("seed", json!(23)),
        ("route", json!("dense")),
        ("status", json!({"state": "succeeded"})),
        ("elapsed_us", json!(1)),
        ("cohort", json!("unknown")),
        ("warm_up", json!(false)),
        ("language", json!("en")),
        ("cross_lingual", json!(false)),
    ];
    for (field, field_value) in &fields {
        let mut value = serde_json::to_value(report_of(Vec::new())).unwrap();
        let mut question = source["questions"][0].clone();
        for (other, _) in &fields {
            question.as_object_mut().unwrap().remove(*other);
        }
        question[*field] = field_value.clone();
        value["questions"] = json!([question]);
        assert!(Report::from_str(&value.to_string()).is_err(), "{field}");
    }
}

#[test]
fn v2_reports_refuse_empty_duplicate_or_inconsistent_question_schedules() {
    let mut empty = report();
    empty.questions.clear();
    assert!(refuses(empty));

    let mut duplicate = report();
    duplicate.questions.push(duplicate.questions[0].clone());
    assert!(refuses(duplicate));

    let mut metadata_mismatch = report();
    let mut other_question = metadata_mismatch.questions[0].clone();
    other_question.id = "other".to_owned();
    other_question.seed = Some(24);
    metadata_mismatch.questions.push(other_question);
    assert!(refuses(metadata_mismatch));

    let mut schedule_mismatch = report();
    let mut other_attempt = schedule_mismatch.questions[0].clone();
    other_attempt.id = "other".to_owned();
    other_attempt.attempt = Some(2);
    other_attempt.repetition = Some(2);
    other_attempt.seed = Some(24);
    schedule_mismatch.questions.push(other_attempt);
    assert!(refuses(schedule_mismatch));
}

#[test]
fn v2_reports_refuse_invalid_item_metadata_even_when_summaries_match() {
    let invalid_rows: [fn(&mut QuestionResult); 10] = [
        |row| row.attempt = Some(0),
        |row| row.seed = None,
        |row| row.status = None,
        |row| {
            row.status = Some(ItemStatus::Failed {
                kind: String::new(),
                reason: "failure".to_owned(),
            });
        },
        |row| {
            row.status = Some(ItemStatus::Failed {
                kind: "retrieval".to_owned(),
                reason: "  ".to_owned(),
            });
        },
        |row| row.elapsed_us = None,
        |row| row.cohort = None,
        |row| row.language = Some("de".to_owned()),
        |row| row.language = None,
        |row| row.cross_lingual = None,
    ];
    for invalidate in invalid_rows {
        let mut report = report();
        invalidate(&mut report.questions[0]);
        assert!(refuses(report));
    }
}

#[test]
fn retrieval_failures_are_not_integrity_violations() {
    let report = run_v2(
        header_v2(),
        &suite(&["question"]),
        |_| Ok::<_, &str>(None),
        |_| Err("timeout"),
    )
    .unwrap();

    assert_eq!(report.integrity_violation, Some(false));
    assert!(!report.questions[0].is_integrity_violation());
}

#[test]
fn v2_reports_compare_successful_throughput_bits_when_read_back() {
    let mut report = report();
    let row = &mut report.questions[0];
    row.elapsed_us = Some(100);
    row.latency_us = 100;
    row.degraded = false;
    row.status = Some(ItemStatus::Succeeded);
    row.cohort = Some(MeasurementCohort::Warm);
    refresh_summaries(&mut report);
    report
        .cohorts
        .as_mut()
        .unwrap()
        .get_mut(&MeasurementCohort::Warm)
        .unwrap()
        .successful_per_second = Some(1.0);
    assert!(Report::from_str(&serde_json::to_string(&report).unwrap()).is_err());
}

#[test]
fn cohort_maps_report_their_expected_shape_for_wrong_json_types() {
    let mut value: Value = serde_json::to_value(report()).unwrap();
    value["cohorts"] = json!([]);
    let error = Report::from_str(&value.to_string())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("an object of unique keys and values"),
        "{error}"
    );
}
