//! Report v2 schedule, subgroup, and timing statistics.

use super::{
    support::bundle,
    v2::{assert_close, header_v2, suite},
};
use crate::{
    eval::{
        HeaderV2, ItemStatus, MeasurementCohort, Report, Subgroup, merge_v2_attempts,
        metric::{measure_attempts, measure_cohorts, measure_subgroups},
        run_v2,
    },
    suite::Suite,
};

#[test]
fn v2_subgroups_use_frozen_cross_lingual_labels_and_language_counts() {
    let suite: Suite = [("fr", "fr"), ("en", "en"), ("cross", "en")]
        .into_iter()
        .map(|(id, language)| {
            serde_json::json!({
                "schema": "maestro-suite/1",
                "id": id,
                "language": language,
                "question": format!("question {id}"),
                "answerable": false,
                "expected": [],
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .parse()
        .unwrap();
    let mut header = header_v2();
    header.cross_lingual_questions.insert("cross".to_owned());
    let report = run_v2(header, &suite, |_| Ok::<_, &str>(None), |_| Ok(bundle(&[]))).unwrap();
    let subgroups = report.subgroups.unwrap();
    assert_eq!(subgroups[&Subgroup::Fr].question_count, 1);
    assert_eq!(subgroups[&Subgroup::En].question_count, 2);
    assert_eq!(subgroups[&Subgroup::CrossLingual].question_count, 1);
    assert_eq!(report.questions[2].cross_lingual, Some(true));
}

#[test]
fn v2_subgroup_statistics_count_warmups_and_ranked_attempts() {
    let suite = suite(&["warm", "ranked-a", "ranked-b"]);
    let report = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let mut rows = report.questions;
    rows[0].warm_up = Some(true);

    let stats = &measure_subgroups(&rows, 42)[&Subgroup::En];
    assert_eq!(stats.question_count, 3);
    assert_eq!(stats.unanswerable_questions, 3);
    assert_eq!(stats.attempt_count, 3);
    assert_eq!(stats.warm_up_attempts, 1);
    assert_eq!(stats.ranked_attempts, 2);
}

#[test]
fn merged_attempt_reports_keep_warmup_costs_and_score_only_ranked_rows() {
    let suite = suite(&["first", "second"]);
    let mut warm_header = header_v2();
    warm_header.repetition = 0;
    warm_header.planned_warm_ups = 1;
    warm_header.warm_up = true;
    let warm = run_v2(
        warm_header,
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let mut scored_header = header_v2();
    scored_header.planned_warm_ups = 1;
    scored_header.attempt = 2;
    scored_header.attempt_seed = 24;
    let scored = run_v2(
        scored_header,
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let merged = merge_v2_attempts(&[warm, scored]).unwrap();
    assert_eq!(merged.questions.len(), 4);
    assert_close(merged.metrics.no_answer_accuracy.unwrap().value, 1.0);
    let warm = &merged.cohorts.as_ref().unwrap()[&MeasurementCohort::Unknown];
    assert_eq!(warm.attempts, 4);
    assert_eq!(warm.warm_up_attempts, 2);
    assert_eq!(warm.timed_samples, 2);
    assert_eq!(warm.succeeded, 4);
    assert_eq!(
        merged.subgroups.as_ref().unwrap()[&Subgroup::En].attempt_count,
        4
    );
}

#[test]
fn v2_merge_requires_retry_metadata_to_match_an_original_scored_attempt() {
    let suite = suite(&["first"]);
    let first = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Err("timeout"),
    )
    .unwrap();
    let mut retry_header = header_v2();
    retry_header.attempt = 2;
    retry_header.retry_of = Some(1);
    retry_header.attempt_seed = 24;
    let retry = run_v2(
        retry_header.clone(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let merged = merge_v2_attempts(&[first.clone(), retry]).unwrap();
    assert_eq!(merged.questions.len(), 2);
    retry_header.retry_of = Some(9);
    let orphan = run_v2(
        retry_header,
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    assert!(merge_v2_attempts(&[first, orphan]).is_err());
}

#[test]
fn v2_merge_requires_every_planned_repetition_and_warmup() {
    let mut two_repetitions = header_v2();
    two_repetitions.planned_repetitions = 2;
    let trailing = attempt_report(two_repetitions.clone(), 1, 1, false, false);
    assert!(merge_v2_attempts(&[trailing]).is_err());
    let first = attempt_report(two_repetitions.clone(), 1, 1, false, false);
    let second = attempt_report(two_repetitions.clone(), 3, 2, false, false);
    assert!(merge_v2_attempts(&[first, second]).is_err());

    let mut three_repetitions = header_v2();
    three_repetitions.planned_repetitions = 3;
    let first = attempt_report(three_repetitions.clone(), 1, 1, false, false);
    let third = attempt_report(three_repetitions.clone(), 2, 3, false, false);
    assert!(merge_v2_attempts(&[first, third]).is_err());

    let duplicate_first = attempt_report(two_repetitions.clone(), 1, 1, false, false);
    let duplicate_second = attempt_report(two_repetitions.clone(), 2, 1, false, false);
    assert!(merge_v2_attempts(&[duplicate_first, duplicate_second]).is_err());

    let first = attempt_report(two_repetitions.clone(), 1, 1, false, false);
    let mut different_plan = two_repetitions;
    different_plan.planned_repetitions = 3;
    let second = attempt_report(different_plan, 2, 2, false, false);
    assert!(merge_v2_attempts(&[first, second]).is_err());

    let mut with_warmup = header_v2();
    with_warmup.planned_warm_ups = 1;
    let scored = attempt_report(with_warmup, 1, 1, false, false);
    assert!(merge_v2_attempts(&[scored]).is_err());
}

#[test]
fn cohort_latency_excludes_failed_and_degraded_attempts_but_keeps_their_costs() {
    let suite = suite(&["success"]);
    let report = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let mut success = report.questions[0].clone();
    success.elapsed_us = Some(100);
    success.latency_us = 100;
    success.cohort = Some(MeasurementCohort::Warm);
    success.status = Some(ItemStatus::Succeeded);
    success.degraded = false;

    let mut large_success = success.clone();
    large_success.id = "large-success".to_owned();
    large_success.attempt = Some(2);
    large_success.elapsed_us = Some(4_294_967_296);
    large_success.latency_us = u32::MAX;

    let mut failed = success.clone();
    failed.id = "failed".to_owned();
    failed.attempt = Some(3);
    failed.elapsed_us = Some(200);
    failed.latency_us = 200;
    failed.status = Some(ItemStatus::Failed {
        kind: "retrieval".to_owned(),
        reason: "synthetic failure".to_owned(),
    });

    let mut degraded = success.clone();
    degraded.id = "degraded".to_owned();
    degraded.attempt = Some(4);
    degraded.elapsed_us = Some(300);
    degraded.latency_us = 300;
    degraded.degraded = true;

    let statistics =
        &measure_cohorts(&[success, large_success, failed, degraded])[&MeasurementCohort::Warm];
    assert_eq!(statistics.attempts, 4);
    assert_eq!(statistics.succeeded, 3);
    assert_eq!(statistics.failed, 1);
    assert_eq!(statistics.timed_samples, 2);
    assert_eq!(statistics.total_elapsed_us, 4_294_967_896);
    assert_eq!(statistics.p50_us, Some(100));
    assert_eq!(statistics.p95_us, Some(4_294_967_296));
    assert_eq!(
        statistics.successful_per_second,
        Some(2_000_000.0 / 4_294_967_396.0)
    );
}

#[test]
fn clustered_attempts_exclude_warmups_but_keep_failure_costs() {
    let suite = suite(&["empty"]);
    let report = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let mut scored = report.questions[0].clone();
    scored.elapsed_us = Some(100);
    scored.latency_us = 100;
    scored.cohort = Some(MeasurementCohort::Warm);
    let mut warmup = scored.clone();
    warmup.attempt = Some(2);
    warmup.seed = Some(24);
    warmup.warm_up = Some(true);
    warmup.status = Some(ItemStatus::Failed {
        kind: "warmup".to_owned(),
        reason: "warm-up failure remains accounted".to_owned(),
    });
    warmup.elapsed_us = Some(300);
    warmup.latency_us = 300;
    warmup.degraded = true;

    let rows = [scored, warmup];
    let metrics = measure_attempts(&rows, 42);
    assert_close(metrics.no_answer_accuracy.unwrap().value, 1.0);
    let stats = &measure_cohorts(&rows)[&MeasurementCohort::Warm];
    assert_eq!(stats.attempts, 2);
    assert_eq!(stats.warm_up_attempts, 1);
    assert_eq!(stats.succeeded, 1);
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.timed_samples, 1);
    assert_eq!(stats.total_elapsed_us, 400);
    assert_eq!(stats.p50_us, Some(100));
    assert_eq!(stats.p95_us, Some(100));
    assert_eq!(stats.successful_per_second, Some(10_000.0));
}

fn attempt_report(
    mut header: HeaderV2,
    attempt: u32,
    repetition: u32,
    warm_up: bool,
    fail: bool,
) -> Report {
    header.attempt = attempt;
    header.repetition = repetition;
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
