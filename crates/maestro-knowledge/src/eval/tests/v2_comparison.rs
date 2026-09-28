//! Report v2 paired comparison rules.

use super::{
    support::bundle,
    v2::{assert_close, header_v2, suite},
};
use crate::eval::{CompareError, ItemStatus, QuestionResult, Report, compare, run_v2};
use maestro_kernel::artifact::Digest;

#[test]
fn v2_comparison_pools_unequal_retries_within_paired_question_clusters() {
    let suite = suite(&["empty"]);
    let mut baseline = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let candidate = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    baseline.questions[0].status = Some(ItemStatus::Failed {
        kind: "retrieval".to_owned(),
        reason: "timeout".to_owned(),
    });
    baseline.questions[0].degraded = true;
    let mut retry = baseline.questions[0].clone();
    retry.attempt = Some(2);
    retry.retry_of = Some(1);
    retry.seed = Some(24);
    retry.status = Some(ItemStatus::Succeeded);
    retry.degraded = false;
    baseline.questions.push(retry);

    let paired = compare(&baseline, &candidate, 42).unwrap();
    assert_eq!(paired.questions, 1);
    assert_eq!(paired.baseline_attempts, 2);
    assert_eq!(paired.candidate_attempts, 1);
    assert_close(paired.differences.no_answer_accuracy.unwrap().value, 0.5);
}

#[test]
fn v2_comparison_rejects_changed_question_labels() {
    let suite = suite(&["empty"]);
    let baseline = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();

    let mut candidate = baseline.clone();
    candidate.questions[0].language = Some("fr".to_owned());
    assert!(matches!(
        compare(&baseline, &candidate, 42),
        Err(CompareError::Labels { ref id }) if id == "empty"
    ));

    let mut candidate = baseline.clone();
    candidate.questions[0].cross_lingual = Some(true);
    assert!(matches!(
        compare(&baseline, &candidate, 42),
        Err(CompareError::Labels { ref id }) if id == "empty"
    ));
}

#[test]
fn v2_comparison_rejects_changed_attempt_fields() {
    let suite = suite(&["empty"]);
    let baseline = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();

    let mut candidate = baseline.clone();
    candidate.questions[0].repetition = Some(2);
    assert!(matches!(
        compare(&baseline, &candidate, 42),
        Err(CompareError::Attempts { ref id }) if id == "empty"
    ));

    let mut candidate = baseline.clone();
    candidate.questions[0].retry_of = Some(1);
    assert!(matches!(
        compare(&baseline, &candidate, 42),
        Err(CompareError::Attempts { ref id }) if id == "empty"
    ));

    let mut candidate = baseline.clone();
    candidate.questions[0].warm_up = Some(true);
    assert!(matches!(
        compare(&baseline, &candidate, 42),
        Err(CompareError::Attempts { ref id }) if id == "empty"
    ));
}

#[test]
fn v2_comparison_pairs_schedule_not_attempt_ids_and_rejects_other_plans() {
    let suite = suite(&["empty"]);
    let baseline = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();

    let mut moved_attempt = baseline.clone();
    moved_attempt.questions[0].attempt = Some(8);
    assert!(compare(&baseline, &moved_attempt, 42).is_ok());

    let mut changed_seed = baseline.clone();
    changed_seed.questions[0].seed = Some(999);
    assert!(matches!(
        compare(&baseline, &changed_seed, 42),
        Err(CompareError::Attempts { ref id }) if id == "empty"
    ));

    let mut changed_plan = baseline.clone();
    changed_plan.planned_repetitions = Some(2);
    assert!(matches!(
        compare(&baseline, &changed_plan, 42),
        Err(CompareError::Attempts { ref id }) if id == "empty"
    ));

    let mut changed_run = baseline.clone();
    changed_run.run_id = Some("another-run".to_owned());
    assert!(matches!(
        compare(&baseline, &changed_run, 42),
        Err(CompareError::RunId)
    ));

    let mut changed_route = baseline.clone();
    changed_route.questions[0].route = Some("bm25".to_owned());
    assert!(matches!(
        compare(&baseline, &changed_route, 42),
        Err(CompareError::Route)
    ));
}

#[test]
fn v2_comparison_omits_warmup_only_question_clusters() {
    let suite = suite(&["scored", "warm-only"]);
    let mut baseline = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let mut candidate = baseline.clone();
    for report in [&mut baseline, &mut candidate] {
        let warm_only = report
            .questions
            .iter_mut()
            .find(|row| row.id == "warm-only")
            .unwrap();
        warm_only.attempt = Some(2);
        warm_only.repetition = Some(0);
        warm_only.warm_up = Some(true);
    }

    let paired = compare(&baseline, &candidate, 42).unwrap();
    assert_eq!(paired.questions, 1);
    assert_eq!(paired.baseline_attempts, 1);
    assert_eq!(paired.candidate_attempts, 1);
}

#[test]
fn paired_v2_comparison_rejects_changed_frozen_digests() {
    let suite = suite(&["empty"]);
    let baseline = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let mut changed_corpus = baseline.clone();
    changed_corpus.corpus_digest = Some(Digest::of(b"changed corpus"));
    assert!(compare(&baseline, &changed_corpus, 42).is_err());
    let mut changed_inputs = baseline.clone();
    changed_inputs.input_digest = Some(Digest::of(b"changed canonical inputs"));
    assert!(compare(&baseline, &changed_inputs, 42).is_err());
}

fn report() -> Report {
    run_v2(
        header_v2(),
        &suite(&["question"]),
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap()
}

fn retry(report: &Report) -> QuestionResult {
    let mut row = report.questions[0].clone();
    row.attempt = Some(2);
    row.repetition = Some(1);
    row.retry_of = Some(1);
    row.seed = Some(24);
    row
}

#[test]
fn v2_comparison_requires_a_present_run_id_even_when_both_are_missing() {
    let mut baseline = report();
    baseline.run_id = None;
    let candidate = baseline.clone();

    assert!(matches!(
        compare(&baseline, &candidate, 42),
        Err(CompareError::RunId)
    ));
}

#[test]
fn v2_comparison_rejects_answerability_drift_within_either_run() {
    for inconsistent_baseline in [true, false] {
        let mut baseline = report();
        let mut candidate = report();
        let changed = if inconsistent_baseline {
            &mut baseline
        } else {
            &mut candidate
        };
        let mut retry = retry(changed);
        retry.answerable = !retry.answerable;
        changed.questions.push(retry);

        assert!(matches!(
            compare(&baseline, &candidate, 42),
            Err(CompareError::Answerability { ref id }) if id == "question"
        ));
    }
}

#[test]
fn v2_comparison_rejects_each_label_drift_within_either_run() {
    for inconsistent_baseline in [true, false] {
        for language_mismatch in [true, false] {
            let mut baseline = report();
            let mut candidate = report();
            let changed = if inconsistent_baseline {
                &mut baseline
            } else {
                &mut candidate
            };
            let mut retry = retry(changed);
            if language_mismatch {
                retry.language = Some("fr".to_owned());
            } else {
                retry.cross_lingual = Some(true);
            }
            changed.questions.push(retry);

            assert!(matches!(
                compare(&baseline, &candidate, 42),
                Err(CompareError::Labels { ref id }) if id == "question"
            ));
        }
    }
}

#[test]
fn v2_comparison_rejects_duplicate_attempts_even_when_both_runs_match() {
    let mut baseline = report();
    let mut candidate = baseline.clone();
    baseline.questions.push(baseline.questions[0].clone());
    candidate.questions.push(candidate.questions[0].clone());

    assert!(matches!(
        compare(&baseline, &candidate, 42),
        Err(CompareError::Attempts { ref id }) if id == "question"
    ));
}

#[test]
fn v2_comparison_rejects_repeated_scheduled_repetitions() {
    let baseline = report();
    let mut candidate = report();
    let mut duplicate_schedule = candidate.questions[0].clone();
    duplicate_schedule.attempt = Some(2);
    candidate.questions.push(duplicate_schedule);

    assert!(matches!(
        compare(&baseline, &candidate, 42),
        Err(CompareError::Attempts { ref id }) if id == "question"
    ));
}

#[test]
fn v2_comparison_rejects_candidate_route_drift_after_its_first_question() {
    let suite = suite(&["first", "second"]);
    let baseline = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let mut candidate = baseline.clone();
    candidate.questions[1].route = Some("bm25".to_owned());

    assert!(matches!(
        compare(&baseline, &candidate, 42),
        Err(CompareError::Route)
    ));
}

#[test]
fn v2_comparison_omits_a_cluster_scored_only_by_a_warmup_retry() {
    let mut header = header_v2();
    header.planned_warm_ups = 1;
    header.repetition = 0;
    header.warm_up = true;
    let baseline = run_v2(
        header,
        &suite(&["question"]),
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let mut candidate = baseline.clone();
    let mut retry = retry(&baseline);
    retry.warm_up = Some(false);
    candidate.questions.push(retry);

    let paired = compare(&baseline, &candidate, 42).unwrap();
    assert_eq!(paired.questions, 0);
    assert_eq!(paired.baseline_attempts, 0);
    assert_eq!(paired.candidate_attempts, 0);
}
