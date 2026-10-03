//! Synthetic frozen acceptance fixtures shared by the gate tests.

use crate::{
    eval::graph::{
        AnswerOutcome, ClaimReview, ClaimVerdict, Cohort, FamilyProof, Gate, GraphRoute,
        LatencySample, Operation, QuestionAnswer, QuestionRetrieval, RefusalOutcome, RunEvidence,
        RungDefinition, RungEvidence, judge_runs, score_construction,
    },
    search::SearchConfiguration,
    suite::Suite,
};
use maestro_kernel::artifact::Digest;
use serde_json::json;
use std::time::Duration;

/// Frozen suite includes answerable retrieval items and a separate refusal item.
pub(super) fn retrieval_suite(count: usize) -> Suite {
    let mut rows: Vec<_> = (0..count)
        .map(|index| {
            json!({
                "schema": "maestro-suite/1", "id": format!("q-{index:03}"), "language": "en",
                "question": "Which source?", "answerable": true,
                "expected": [{"source_ref": "synthetic.md", "heading_path": []}]
            })
            .to_string()
        })
        .collect();
    for index in 0..20 {
        rows.push(
            json!({"schema": "maestro-suite/1", "id": format!("u-{index:03}"), "language": "en",
        "question": "Unsupported?", "answerable": false, "expected": []})
            .to_string(),
        );
    }
    rows.join("\n").parse().unwrap()
}

/// `pairs` families, the first `complete` complete.
pub(super) fn proofs(pairs: usize, complete: usize) -> Vec<FamilyProof> {
    (0..pairs)
        .map(|index| FamilyProof {
            family: format!("f-{index:03}"),
            complete: index < complete,
        })
        .collect()
}

/// `count` questions, the first `hits` found at rank 1, the others missed.
pub(super) fn retrieval(count: usize, hits: usize) -> Vec<QuestionRetrieval> {
    (0..count)
        .map(|index| QuestionRetrieval {
            id: format!("q-{index:03}"),
            hit_at_10: index < hits,
            reciprocal_rank: if index < hits { 1.0 } else { 0.0 },
        })
        .collect()
}

/// A rung of `run` with `graph`, S1's default configuration and `found`
/// complete proofs among 200.
pub(super) fn rung(run: u32, graph: GraphRoute, found: usize) -> RungEvidence {
    RungEvidence {
        run,
        definition: RungDefinition {
            configuration: SearchConfiguration::default(),
            reranker: Some(Digest::of(b"reranker")),
            graph,
        },
        proofs: proofs(200, found),
        graph_retrieval: retrieval(200, 60),
        graph_answers: graph_answers(),
        golden_answers: golden_answers(),
        supported_answers: 60,
        ctm_retrieval: retrieval(50, 40),
        refusals: vec![RefusalOutcome::Refused; 20],
    }
}

/// Warm samples of `operation`, every one ending in `elapsed`.
pub(super) fn warm(operation: Operation, elapsed: Duration) -> Vec<LatencySample> {
    (0..20)
        .map(|_| LatencySample {
            cohort: Cohort::Warm,
            operation,
            elapsed: Some(elapsed),
        })
        .collect()
}

/// A run that passes every gate: pairing finds ten more complete proofs.
pub(super) fn passing(run: u32) -> RunEvidence {
    let reviews: Vec<ClaimReview> = (0..20)
        .map(|index| ClaimReview {
            claim_id: format!("claim-{index}"),
            verdict: ClaimVerdict::Correct,
            quote_exact: true,
        })
        .collect();
    let mut latency = warm(Operation::Graph, Duration::from_millis(400));
    for operation in [
        Operation::Neighbors,
        Operation::Path,
        Operation::EntityResolve,
        Operation::EvidenceTrace,
    ] {
        latency.extend(warm(operation, Duration::from_millis(400)));
    }
    latency.extend(warm(Operation::Search, Duration::from_millis(2_000)));
    latency.extend(warm(Operation::Ask, Duration::from_secs(9)));
    let mut graph_only = rung(run, GraphRoute::Enabled, 40);
    graph_only.definition.configuration = SearchConfiguration {
        dense_enabled: false,
        lexical_enabled: false,
        identifier_enabled: false,
        structured_enabled: false,
        ..SearchConfiguration::default()
    };
    RunEvidence {
        run,
        passage_only: rung(run, GraphRoute::None, 40),
        graph_only,
        pairing: rung(run, GraphRoute::Enabled, 50),
        construction: score_construction(&reviews, &[], &[]).unwrap(),
        inexact_commands: 0,
        latency,
    }
}

/// The gates `evidence` fails in its one run.
pub(super) fn failed(evidence: RunEvidence) -> Vec<Gate> {
    let verdict = judge_runs(
        &[evidence],
        &retrieval_suite(200),
        &retrieval_suite(50),
        &golden_suite(),
    );
    verdict.runs[0].failed_gates()
}

/// Golden keeps 84 answerable entries and 16 unanswerables.
pub(super) fn golden_suite() -> Suite {
    (0..100)
        .map(|index| {
            let expected = if index < 84 {
                vec![json!({"source_ref":"synthetic.md", "heading_path":[]})]
            } else {
                vec![]
            };
            json!({
                "schema":"maestro-suite/1", "id":format!("g-{index:03}"), "language":"en",
                "question":"Synthetic Golden?", "answerable":index < 84,
                "expected":expected
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .parse()
        .unwrap()
}

/// Every Golden entry answered correctly.
pub(super) fn golden_answers() -> Vec<QuestionAnswer> {
    (0..100)
        .map(|index| QuestionAnswer {
            id: format!("g-{index:03}"),
            outcome: if index < 84 {
                AnswerOutcome::Supported
            } else {
                AnswerOutcome::Refused
            },
        })
        .collect()
}

/// All 220 graph items have an attempted final answer.
pub(super) fn graph_answers() -> Vec<QuestionAnswer> {
    (0..200)
        .map(|index| QuestionAnswer {
            id: format!("q-{index:03}"),
            outcome: AnswerOutcome::Supported,
        })
        .chain((0..20).map(|index| QuestionAnswer {
            id: format!("u-{index:03}"),
            outcome: AnswerOutcome::Refused,
        }))
        .collect()
}
