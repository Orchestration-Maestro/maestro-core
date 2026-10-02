//! The gate rules of the spec's table: pairing against same-run
//! passage-only, every gate in each of three runs, the refusal denominator,
//! point-estimate non-regression and warm latency cohorts.

use crate::eval::graph::{
    ClaimReview, ClaimVerdict, Cohort, Gate, GraphRoute, LatencySample, Operation, RefusalOutcome,
    judge_runs, score_construction,
};
use maestro_kernel::artifact::Digest;
use std::time::Duration;

use super::gates_support::{
    failed, golden_suite, passing, proofs, retrieval, retrieval_suite, warm,
};

#[test]
fn three_passing_runs_pass_and_fewer_or_a_failing_one_do_not() {
    let runs = [passing(1), passing(2), passing(3)];
    assert!(
        judge_runs(
            &runs,
            &retrieval_suite(200),
            &retrieval_suite(50),
            &golden_suite()
        )
        .passed
    );
    assert!(
        !judge_runs(
            &runs[..2],
            &retrieval_suite(200),
            &retrieval_suite(50),
            &golden_suite()
        )
        .passed,
        "two runs are missing evidence"
    );
    let mut one_fails = runs.clone();
    one_fails[1].pairing.proofs = proofs(200, 49);
    let verdict = judge_runs(
        &one_fails,
        &retrieval_suite(200),
        &retrieval_suite(50),
        &golden_suite(),
    );
    assert!(!verdict.passed);
    assert_eq!(verdict.runs[1].failed_gates(), vec![Gate::ProofGain]);
    assert!(verdict.runs[0].passed && verdict.runs[2].passed);
    let repeated = [passing(1), passing(1), passing(2)];
    assert!(
        !judge_runs(
            &repeated,
            &retrieval_suite(200),
            &retrieval_suite(50),
            &golden_suite()
        )
        .passed,
        "a run counted twice"
    );
}

#[test]
fn pairing_is_compared_with_passage_only_of_the_same_run() {
    let mut evidence = passing(1);
    evidence.passage_only.run = 2;
    assert_eq!(failed(evidence), vec![Gate::SameRunComparator]);
}

#[test]
fn passage_only_is_s1_defaults_with_graph_none_and_the_same_reranker() {
    let mut graph = passing(1);
    graph.passage_only.definition.graph = GraphRoute::Enabled;
    assert_eq!(failed(graph), vec![Gate::PassageDefinition]);
    let mut weights = passing(1);
    weights.passage_only.definition.configuration.lexical_weight = 2.0;
    assert_eq!(failed(weights), vec![Gate::PassageDefinition]);
    let mut routes = passing(1);
    routes
        .passage_only
        .definition
        .configuration
        .identifier_enabled = false;
    assert_eq!(failed(routes), vec![Gate::PassageDefinition]);
    let mut other = passing(1);
    other.passage_only.definition.reranker = Some(Digest::of(b"another"));
    assert_eq!(failed(other), vec![Gate::PassageDefinition]);
    let mut pairing = passing(1);
    pairing.pairing.definition.graph = GraphRoute::None;
    assert_eq!(failed(pairing), vec![Gate::PassageDefinition]);
}

#[test]
fn relation_precision_and_exactness_gate_each_run() {
    let mut imprecise = passing(1);
    imprecise.construction.correct = 18;
    imprecise.construction.precision_passed = false;
    assert_eq!(failed(imprecise), vec![Gate::RelationPrecision]);
    let mut inexact = passing(1);
    inexact.inexact_commands = 1;
    assert_eq!(failed(inexact), vec![Gate::Exactness]);
    let mut quotes = passing(1);
    quotes.construction.exact = false;
    assert_eq!(failed(quotes), vec![Gate::Exactness]);
}

#[test]
fn refusal_counts_every_unanswerable_question_and_errors_earn_no_credit() {
    let mut sixteen = passing(1);
    let pairing = &mut sixteen.pairing.refusals;
    for outcome in pairing.iter_mut().take(4) {
        *outcome = RefusalOutcome::TimedOut;
    }
    assert!(failed(sixteen).is_empty());
    let mut fifteen = passing(1);
    let pairing = &mut fifteen.pairing.refusals;
    for outcome in pairing.iter_mut().take(5) {
        *outcome = RefusalOutcome::Failed;
    }
    assert_eq!(failed(fifteen), vec![Gate::Refusal]);
    let mut answered = passing(1);
    answered.pairing.refusals[..5].fill(RefusalOutcome::Answered);
    assert_eq!(failed(answered), vec![Gate::Refusal]);
    let mut none = passing(1);
    none.pairing.refusals.clear();
    assert_eq!(failed(none), vec![Gate::Refusal]);
}

#[test]
fn pairing_must_not_lose_ctm_graph_retrieval_or_supported_answers() {
    let mut recall = passing(1);
    recall.pairing.graph_retrieval = retrieval(200, 59);
    assert_eq!(failed(recall), vec![Gate::GraphRetrieval]);
    let mut supported = passing(1);
    supported.pairing.supported_answers = 59;
    assert_eq!(failed(supported), vec![Gate::GraphRetrieval]);
    let mut mrr = passing(1);
    mrr.pairing.graph_retrieval[0].reciprocal_rank = 0.5;
    assert_eq!(failed(mrr), vec![Gate::GraphRetrieval]);
}

#[test]
fn a_negative_ctm_retrieval_delta_fails_and_a_nonnegative_one_passes_whatever_its_interval() {
    let mut worse = passing(1);
    worse.pairing.ctm_retrieval[0].reciprocal_rank = 0.5;
    assert_eq!(failed(worse), vec![Gate::CtmRetrieval]);
    let mut lost = passing(1);
    lost.pairing.ctm_retrieval[0].hit_at_10 = false;
    assert_eq!(failed(lost), vec![Gate::CtmRetrieval]);
    // One question gained and one lost: a zero delta whose interval crosses
    // zero passes, as the interval is diagnostic only.
    let mut swapped = passing(1);
    swapped.pairing.ctm_retrieval[0].hit_at_10 = false;
    swapped.pairing.ctm_retrieval[0].reciprocal_rank = 0.0;
    swapped.pairing.ctm_retrieval[45].hit_at_10 = true;
    swapped.pairing.ctm_retrieval[45].reciprocal_rank = 1.0;
    let verdict = judge_runs(
        &[swapped],
        &retrieval_suite(200),
        &retrieval_suite(50),
        &golden_suite(),
    );
    assert!(verdict.runs[0].failed_gates().is_empty());
    let interval = verdict.runs[0].ctm_recall_interval.unwrap();
    assert!(interval[0] < 0.0 && interval[1] > 0.0, "{interval:?}");
    let mut unpaired = passing(1);
    unpaired.pairing.ctm_retrieval.pop();
    assert_eq!(failed(unpaired), vec![Gate::CtmRetrieval]);
}

#[test]
fn only_warm_samples_count_against_the_latency_limits() {
    let mut cold = passing(1);
    cold.latency.extend((0..40).map(|_| LatencySample {
        cohort: Cohort::Cold,
        operation: Operation::Graph,
        elapsed: Some(Duration::from_secs(5)),
    }));
    cold.latency.push(LatencySample {
        cohort: Cohort::Unavailable,
        operation: Operation::Ask,
        elapsed: None,
    });
    assert!(failed(cold).is_empty());
    let mut slow = passing(1);
    slow.latency
        .extend(warm(Operation::Graph, Duration::from_millis(501)));
    assert_eq!(failed(slow), vec![Gate::Latency]);
    let mut search = passing(1);
    search
        .latency
        .retain(|sample| sample.operation != Operation::Search);
    search
        .latency
        .extend(warm(Operation::Search, Duration::from_millis(2_500)));
    assert_eq!(failed(search), vec![Gate::Latency], "search is under 2.5 s");
    let mut timeouts = passing(1);
    timeouts.latency.extend((0..3).map(|_| LatencySample {
        cohort: Cohort::Warm,
        operation: Operation::Ask,
        elapsed: None,
    }));
    assert_eq!(failed(timeouts), vec![Gate::Latency]);
    let mut missing = passing(1);
    missing
        .latency
        .retain(|sample| sample.operation != Operation::Ask);
    assert_eq!(failed(missing), vec![Gate::Latency]);
}

#[test]
fn missing_answerable_families_and_duplicate_retrieval_items_block_acceptance() {
    let mut short = passing(1);
    short.passage_only.proofs = proofs(199, 39);
    short.pairing.proofs = proofs(199, 49);
    assert_eq!(failed(short), vec![Gate::ProofGain]);
    let mut duplicate = passing(1);
    duplicate.passage_only.ctm_retrieval[1].id = "q-000".to_owned();
    duplicate.pairing.ctm_retrieval[1].id = "q-000".to_owned();
    assert_eq!(failed(duplicate), vec![Gate::CtmRetrieval]);
}

#[test]
fn incorrect_construction_quotes_still_fail_exactness() {
    let mut evidence = passing(1);
    let reviews: Vec<_> = (0..20)
        .map(|index| ClaimReview {
            claim_id: format!("claim-{index}"),
            verdict: if index == 19 {
                ClaimVerdict::Incorrect
            } else {
                ClaimVerdict::Correct
            },
            quote_exact: index != 19,
        })
        .collect();
    evidence.construction = score_construction(&reviews, &[], &[]).unwrap();
    assert!(evidence.construction.precision_passed);
    assert_eq!(failed(evidence), vec![Gate::Exactness]);
}

#[test]
fn matching_missing_or_unknown_retrieval_rows_are_refused() {
    for replacement in [None, Some("unknown"), Some("unanswerable")] {
        let mut evidence = passing(1);
        for rung in [&mut evidence.passage_only, &mut evidence.pairing] {
            if let Some(id) = replacement {
                rung.graph_retrieval[0].id = id.to_owned();
                rung.ctm_retrieval[0].id = id.to_owned();
            } else {
                rung.graph_retrieval.remove(0);
                rung.ctm_retrieval.remove(0);
            }
        }
        assert_eq!(
            failed(evidence),
            vec![Gate::GraphRetrieval, Gate::CtmRetrieval]
        );
    }
}

#[test]
fn graph_p95_allows_500_ms_and_one_outlier_but_ask_rejects_ten_seconds() {
    let mut evidence = passing(1);
    evidence
        .latency
        .retain(|sample| sample.operation != Operation::Graph);
    let mut graph = warm(Operation::Graph, Duration::from_millis(500));
    graph[19].elapsed = Some(Duration::from_millis(501));
    evidence.latency.extend(graph);
    assert!(failed(evidence.clone()).is_empty());
    evidence
        .latency
        .retain(|sample| sample.operation != Operation::Ask);
    evidence
        .latency
        .extend(warm(Operation::Ask, Duration::from_secs(10)));
    assert_eq!(failed(evidence), vec![Gate::Latency]);
}

#[test]
fn each_graph_tool_requires_its_own_passing_warm_population() {
    for operation in [
        Operation::Neighbors,
        Operation::Path,
        Operation::EntityResolve,
        Operation::EvidenceTrace,
    ] {
        let mut evidence = passing(1);
        evidence
            .latency
            .extend(warm(Operation::Graph, Duration::from_millis(1)));
        evidence
            .latency
            .retain(|sample| sample.operation != operation);
        evidence.latency.push(LatencySample {
            operation,
            cohort: Cohort::Warm,
            elapsed: Some(Duration::from_millis(501)),
        });
        assert_eq!(failed(evidence.clone()), vec![Gate::Latency]);
        evidence
            .latency
            .retain(|sample| sample.operation != operation);
        assert_eq!(failed(evidence), vec![Gate::Latency]);
    }
}
