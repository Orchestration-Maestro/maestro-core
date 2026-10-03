//! Independent population and diagnostic boundaries from the slice mutation misses.
use super::gates_support::{golden_suite, passing, retrieval_suite};
use super::support::{SOURCE, SOURCE_REF, answerable, check, labels, link, text};
use crate::eval::graph::completeness::request_completeness;
use crate::eval::graph::{
    AnswerOutcome, CheckError, Gate, RefusalOutcome, Stage, check_labels, judge_runs,
};
use maestro_kernel::artifact::Digest;
use serde_json::json;
use std::{convert::Infallible, slice::from_ref};

#[test]
fn completeness_checks_each_suite_invariant_independently() {
    for golden in [false, true] {
        for defect in 0..4 {
            let mut graph = retrieval_suite(200);
            let mut gold = golden_suite();
            let suite = if golden { &mut gold } else { &mut graph };
            match defect {
                0 => {
                    suite.questions.pop();
                }
                1 => suite.questions[0].answerable = false,
                2 => suite.questions[0].id = "unsafe id".into(),
                _ => suite.questions[0].id = suite.questions[1].id.clone(),
            }
            assert!(
                !request_completeness(&graph, &gold, &[]).passed,
                "golden={golden} defect={defect}"
            );
        }
    }
    assert!(request_completeness(&retrieval_suite(200), &golden_suite(), &[]).passed);
}

#[test]
fn run_method_and_exact_refusal_threshold_match_the_verdict() {
    let mut evidence = passing(1);
    evidence.pairing.refusals[..4].fill(RefusalOutcome::Failed);
    for failed in [false, true] {
        evidence.inexact_commands = usize::from(failed);
        let verdict = judge_runs(
            &[evidence.clone()],
            &retrieval_suite(200),
            &retrieval_suite(50),
            &golden_suite(),
        );
        assert_eq!(verdict.runs[0].passed(), !failed);
    }
}

#[test]
fn paired_interval_is_exact_for_constant_positive_and_negative_deltas() {
    for (base, candidate, expected) in [(false, true, 1.0), (true, false, -1.0)] {
        let mut run = passing(1);
        for row in &mut run.passage_only.ctm_retrieval {
            row.hit_at_10 = base;
        }
        for row in &mut run.pairing.ctm_retrieval {
            row.hit_at_10 = candidate;
        }
        let verdict = judge_runs(
            &[run],
            &retrieval_suite(200),
            &retrieval_suite(50),
            &golden_suite(),
        );
        assert_eq!(
            verdict.runs[0].ctm_recall_interval,
            Some([expected, expected])
        );
    }
    for baseline in [false, true] {
        let mut run = passing(1);
        let rows = if baseline {
            &mut run.passage_only.ctm_retrieval
        } else {
            &mut run.pairing.ctm_retrieval
        };
        rows.pop();
        let verdict = judge_runs(
            &[run],
            &retrieval_suite(200),
            &retrieval_suite(50),
            &golden_suite(),
        );
        assert_eq!(verdict.runs[0].ctm_recall_interval, None);
    }
}

#[test]
fn label_errors_report_exact_line_numbers_and_summary_counts() {
    let checked = check(&labels(), Stage::Frozen).unwrap();
    assert_eq!(
        (checked.summary.answerable, checked.summary.unanswerable),
        (2, 1)
    );
    for (line, replacement) in [(2, ""), (1, ""), (2, "not json")] {
        let mut rows: Vec<_> = text(&labels()).lines().map(str::to_owned).collect();
        rows[line - 1] = replacement.into();
        let input = rows.join("\n");
        let error = check_labels(
            &super::support::suite(),
            &input,
            &Digest::of(input.as_bytes()),
            Stage::Draft,
            |reference, digest| Ok::<_, Infallible>(super::support::source(reference, digest)),
        )
        .unwrap_err();
        let CheckError::Label(error) = error;
        assert_eq!(error.line, line);
    }
}

#[test]
fn link_endpoints_refuse_independently_and_source_end_is_inclusive() {
    for field in ["subject", "object"] {
        let mut rows = labels();
        rows[0]["proofs"][0]["links"][0][field] = json!("");
        assert!(check(&rows, Stage::Frozen).is_err(), "{field}");
    }
    let mut rows = labels();
    let quote = SOURCE.trim_end().rsplit('\n').next().unwrap();
    let quote = format!("{quote}\n");
    rows[0] = answerable(
        "q-1",
        "f-1",
        "relationship",
        &[vec![link("lantern", "PART_OF", "beacon", &[&quote])]],
    );
    assert!(check(&rows, Stage::Frozen).is_ok());
    assert_eq!(
        rows[0]["proofs"][0]["links"][0]["anchors"][0]["source_ref"],
        SOURCE_REF
    );
}

#[test]
fn label_error_zero_line_stays_file_level() {
    use crate::eval::graph::{LabelCode, LabelError};
    let error = LabelError {
        code: LabelCode::Malformed,
        line: 0,
        item: None,
    };
    assert_eq!(error.to_string(), "malformed");
}

#[test]
fn proof_gain_reports_empty_for_either_empty_arm() {
    use crate::eval::graph::{FamilyProof, GainError, proof_gain};
    let row = FamilyProof {
        family: "family".into(),
        complete: true,
    };
    assert_eq!(
        proof_gain(&[], from_ref(&row), 0).unwrap_err(),
        GainError::Empty
    );
    assert_eq!(proof_gain(&[row], &[], 0).unwrap_err(), GainError::Empty);
}

#[test]
fn label_summary_counts_two_unanswerables_and_one_answerable() {
    let mut suite = super::support::suite();
    suite.questions.remove(1);
    let mut fourth = suite.questions[1].clone();
    fourth.id = "q-4".into();
    suite.questions.push(fourth);
    let mut rows = labels();
    rows.remove(1);
    rows.push(super::support::unanswerable("q-4", "f-4"));
    let input = text(&rows);
    let checked = check_labels(
        &suite,
        &input,
        &Digest::of(input.as_bytes()),
        Stage::Frozen,
        |reference, digest| Ok::<_, Infallible>(super::support::source(reference, digest)),
    )
    .unwrap();
    assert_eq!(
        (checked.summary.answerable, checked.summary.unanswerable),
        (1, 2)
    );
}

#[test]
fn pairing_configuration_and_reranker_are_frozen_independently() {
    for reranker in [false, true] {
        let mut run = passing(1);
        if reranker {
            run.pairing.definition.reranker = Some(Digest::of(b"different"));
        } else {
            run.pairing.definition.configuration.lexical_weight = 2.0;
        }
        let verdict = judge_runs(
            &[run],
            &retrieval_suite(200),
            &retrieval_suite(50),
            &golden_suite(),
        );
        assert_eq!(verdict.runs[0].failed_gates(), [Gate::PassageDefinition]);
    }
}

#[test]
fn sixteen_credited_graph_refusals_meet_the_exact_threshold() {
    let mut run = passing(1);
    for row in run.pairing.graph_answers.iter_mut().skip(200).take(4) {
        row.outcome = AnswerOutcome::Failed;
    }
    let verdict = judge_runs(
        &[run],
        &retrieval_suite(200),
        &retrieval_suite(50),
        &golden_suite(),
    );
    assert!(verdict.runs[0].passed());
}

#[test]
fn wrong_label_schema_reports_the_second_jsonl_line() {
    let mut rows = labels();
    rows[1]["schema"] = json!("maestro-graph-labels/999");
    let CheckError::Label(error) = check(&rows, Stage::Frozen).unwrap_err();
    assert_eq!(error.line, 2);
}
