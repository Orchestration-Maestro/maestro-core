//! Golden support and refusal gates never substitute retrieval credit.
use super::gates_support::{failed, passing};
use crate::eval::graph::{AnswerOutcome, Gate};

#[test]
fn golden_84_support_and_16_refusal_fail_separately_for_both_rungs() {
    for graph_only in [false, true] {
        for outcome in [
            AnswerOutcome::Unsupported,
            AnswerOutcome::Failed,
            AnswerOutcome::TimedOut,
        ] {
            let mut run = passing(1);
            let rung = if graph_only {
                &mut run.graph_only
            } else {
                &mut run.pairing
            };
            rung.golden_answers[0].outcome = outcome;
            assert_eq!(failed(run), vec![Gate::GoldenSupport]);
            let mut run = passing(1);
            let rung = if graph_only {
                &mut run.graph_only
            } else {
                &mut run.pairing
            };
            rung.golden_answers[99].outcome = outcome;
            assert_eq!(failed(run), vec![Gate::GoldenRefusal]);
        }
    }
    let mut run = passing(1);
    run.pairing.golden_answers[0].outcome = AnswerOutcome::Refused;
    run.pairing.golden_answers[84].outcome = AnswerOutcome::Supported;
    assert_eq!(failed(run), vec![Gate::GoldenSupport, Gate::GoldenRefusal]);
}

#[test]
fn golden_missing_answer_is_not_a_retrieval_only_pass() {
    let mut run = passing(1);
    run.pairing.golden_answers.remove(0);
    assert_eq!(failed(run), vec![Gate::Completeness, Gate::GoldenSupport]);
}

#[test]
fn golden_compares_against_same_run_a_not_an_absolute_floor() {
    let mut run = passing(1);
    for rung in [&mut run.passage_only, &mut run.graph_only, &mut run.pairing] {
        rung.golden_answers[0].outcome = AnswerOutcome::Failed;
        rung.golden_answers[99].outcome = AnswerOutcome::TimedOut;
    }
    assert!(failed(run).is_empty());
}
