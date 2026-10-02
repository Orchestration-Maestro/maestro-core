//! Every suite, rung, repeat and item needs exactly one answered request.
use super::gates_support::{failed, golden_suite, passing, retrieval_suite};
use crate::eval::graph::{AnswerOutcome, Gate, RunEvidence, judge_runs};

#[test]
fn all_2880_requests_are_required_not_retrieval_rows() {
    let mut runs = [passing(1), passing(2), passing(3)];
    assert!(
        judge_runs(
            &runs,
            &retrieval_suite(200),
            &retrieval_suite(50),
            &golden_suite()
        )
        .passed
    );
    for case in 0..18 {
        let repeat = case / 6;
        let arm = (case / 2) % 3;
        let golden = case % 2 == 0;
        let saved = runs.clone();
        remove_request(&mut runs[repeat], arm, golden);
        let verdict = judge_runs(
            &runs,
            &retrieval_suite(200),
            &retrieval_suite(50),
            &golden_suite(),
        );
        assert!(!verdict.passed, "repeat={repeat} arm={arm} golden={golden}");
        assert!(
            verdict.runs[repeat]
                .failed_gates()
                .contains(&Gate::Completeness)
        );
        runs = saved;
    }
}

#[test]
fn duplicate_unknown_and_missing_baseline_requests_refuse() {
    for defect in ["duplicate", "unknown", "missing"] {
        let mut run = passing(1);
        let rows = &mut run.passage_only.graph_answers;
        match defect {
            "duplicate" => rows.push(rows[0].clone()),
            "unknown" => rows[0].id = "unknown".to_owned(),
            _ => {
                rows.pop();
            }
        }
        assert_eq!(failed(run), vec![Gate::Completeness]);
    }
}

#[test]
fn attempted_errors_count_as_requests_but_earn_no_answer_credit() {
    let mut run = passing(1);
    for rung in [&mut run.passage_only, &mut run.graph_only, &mut run.pairing] {
        rung.graph_answers[0].outcome = AnswerOutcome::Failed;
        rung.golden_answers[0].outcome = AnswerOutcome::TimedOut;
    }
    assert!(failed(run).is_empty());
}

#[test]
fn completeness_diagnostics_are_counts_and_safe_item_ids_only() {
    let mut run = passing(1);
    run.graph_only.graph_answers.remove(0);
    run.graph_only
        .golden_answers
        .push(run.graph_only.golden_answers[0].clone());
    run.graph_only.graph_answers[0].id = "private question with spaces".to_owned();
    let verdict = judge_runs(
        &[run],
        &retrieval_suite(200),
        &retrieval_suite(50),
        &golden_suite(),
    );
    let requests = &verdict.runs[0].requests;
    assert_eq!((requests.required, requests.observed), (960, 960));
    assert_eq!(requests.missing_ids, ["q-000", "q-001"]);
    assert_eq!(requests.rejected_ids, ["g-000"]);
    assert_eq!(requests.unsafe_ids, 1);
    assert!(!requests.passed);
    assert!(!format!("{requests:?}").contains("private question"));
    assert_eq!(verdict.requests.required, 2_880);
}

#[test]
fn reduced_suite_populations_and_equal_count_substitutions_still_refuse() {
    let runs = [passing(1), passing(2), passing(3)];
    let mut graph = retrieval_suite(200);
    graph.questions.pop();
    assert!(!judge_runs(&runs, &graph, &retrieval_suite(50), &golden_suite()).passed);
    let mut run = passing(1);
    run.passage_only.graph_answers.pop();
    run.passage_only
        .golden_answers
        .push(run.passage_only.golden_answers[0].clone());
    assert!(failed(run).contains(&Gate::Completeness));
}

/// Drop one answered request from a selected arm and suite.
fn remove_request(run: &mut RunEvidence, arm: usize, golden: bool) {
    let rung = match arm {
        0 => &mut run.passage_only,
        1 => &mut run.graph_only,
        _ => &mut run.pairing,
    };
    let rows = if golden {
        &mut rung.golden_answers
    } else {
        &mut rung.graph_answers
    };
    rows.pop();
}

#[test]
fn passing_population_is_1980_graph_plus_900_golden_requests() {
    let verdict = judge_runs(
        &[passing(1), passing(2), passing(3)],
        &retrieval_suite(200),
        &retrieval_suite(50),
        &golden_suite(),
    );
    assert!(verdict.passed && verdict.requests.passed);
    assert_eq!(
        (verdict.requests.required, verdict.requests.observed),
        (2_880, 2_880)
    );
    for run in verdict.runs {
        assert!(run.requests.passed);
        assert_eq!((run.requests.required, run.requests.observed), (960, 960));
    }
}

#[test]
fn graph_request_failures_earn_no_support_or_refusal_credit() {
    let mut run = passing(1);
    run.pairing.graph_answers[0].outcome = AnswerOutcome::TimedOut;
    assert_eq!(failed(run), vec![Gate::GraphRetrieval]);
    let mut run = passing(1);
    for row in run.pairing.graph_answers.iter_mut().skip(200).take(5) {
        row.outcome = AnswerOutcome::Failed;
    }
    assert_eq!(failed(run), vec![Gate::Refusal]);
}
