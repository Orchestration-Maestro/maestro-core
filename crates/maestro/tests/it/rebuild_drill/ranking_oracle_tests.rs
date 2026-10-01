//! Equality-oracle tests for ordered rankings and frozen search inputs.

use super::ranking_oracle::*;
use maestro_kernel::evidence::{RequestBudget, RouteStatus};
use std::collections::BTreeMap;

fn candidate(id: &str, dense_rank: u32) -> CandidateRanking {
    CandidateRanking {
        chunk_id: id.to_owned(),
        document_id: "document".to_owned(),
        revision_id: "revision".to_owned(),
        source_ref: "https://synthetic.example/document".to_owned(),
        section_id: Some("section".to_owned()),
        span_start: 0,
        span_end: 12,
        original_digest: "original".to_owned(),
        prepared_digest: "prepared".to_owned(),
        ranks: BTreeMap::from([("dense".to_owned(), dense_rank)]),
    }
}

fn question(candidates: Vec<CandidateRanking>) -> QuestionRanking {
    QuestionRanking {
        id: "q1".to_owned(),
        language: "en".to_owned(),
        answerable: true,
        question: "synthetic question".to_owned(),
        expected: serde_json::json!(["expected"]),
        budget: RequestBudget::default(),
        routes: REQUIRED_ROUTES
            .into_iter()
            .map(|route| (route.to_owned(), RouteStatus::Ok))
            .chain([("rerank".to_owned(), RouteStatus::Ok)])
            .collect(),
        inventory: None,
        known_gaps: Vec::new(),
        candidates,
    }
}

fn snapshot(questions: Vec<QuestionRanking>) -> Snapshot {
    Snapshot {
        schema: SCHEMA.to_owned(),
        frozen: Frozen {
            suite_digest: "suite".to_owned(),
            collection_digest: "collection".to_owned(),
            corpus_manifest_digest: "corpus".to_owned(),
            collection: "synthetic".to_owned(),
            chunk_set: "set".to_owned(),
            card_digest: "card".to_owned(),
            reranker_digest: "reranker".to_owned(),
            profiles: BTreeMap::new(),
            grants: vec!["workspace/default".to_owned()],
            backend: BTreeMap::new(),
            tie_policy: "rrf-score-desc-chunk-id-asc".to_owned(),
            model_identity: "fake/1".to_owned(),
            qualification_digest: "qualification".to_owned(),
            rerank_depth: 80,
            point_ids: vec!["point-a".to_owned()],
        },
        generation: 1,
        questions,
    }
}

#[test]
fn oracle_rejects_a_rank_permutation_with_identical_aggregate_metrics() {
    let before = snapshot(vec![question(vec![candidate("a", 1), candidate("b", 2)])]);
    let mut after = snapshot(vec![question(vec![candidate("b", 2), candidate("a", 1)])]);
    after.generation = 2;
    let difference = compare_rankings(&before, &after).unwrap_err();
    assert!(difference.contains("q1"));
    assert!(difference.contains("rank 1"));
}

#[test]
fn oracle_rejects_a_missing_or_duplicate_question() {
    let first = question(vec![candidate("a", 1)]);
    let mut second = first.clone();
    second.id = "q2".to_owned();
    let before = snapshot(vec![first.clone(), second]);
    let mut missing = snapshot(vec![first.clone()]);
    missing.generation = 2;
    assert!(
        compare_rankings(&before, &missing)
            .unwrap_err()
            .contains("question count")
    );
    let duplicate = first.clone();
    let mut after = snapshot(vec![first, duplicate]);
    after.generation = 2;
    assert!(
        compare_rankings(&before, &after)
            .unwrap_err()
            .contains("duplicate question")
    );
}

#[test]
fn oracle_rejects_unequal_candidate_counts_changed_digests_and_degraded_routes() {
    let before = snapshot(vec![question(vec![candidate("a", 1), candidate("b", 2)])]);
    let mut short = snapshot(vec![question(vec![candidate("a", 1)])]);
    short.generation = 2;
    assert!(
        compare_rankings(&before, &short)
            .unwrap_err()
            .contains("candidate count")
    );

    let mut changed = snapshot(vec![question(vec![candidate("a", 1), candidate("b", 2)])]);
    changed.generation = 2;
    changed.questions[0].candidates[0].prepared_digest = "different".to_owned();
    assert!(
        compare_rankings(&before, &changed)
            .unwrap_err()
            .contains("rank 1")
    );

    let mut degraded = snapshot(vec![question(vec![candidate("a", 1)])]);
    degraded.generation = 2;
    degraded.questions[0].routes.insert(
        "dense".to_owned(),
        RouteStatus::Unavailable("lost".to_owned()),
    );
    assert!(
        compare_rankings(&before, &degraded)
            .unwrap_err()
            .contains("degraded")
    );
}

#[test]
fn oracle_rejects_changed_span_and_changed_question_digest() {
    let before = snapshot(vec![question(vec![candidate("a", 1)])]);
    let mut changed = snapshot(vec![question(vec![candidate("a", 1)])]);
    changed.generation = 2;
    changed.questions[0].candidates[0].span_end += 1;
    assert!(
        compare_rankings(&before, &changed)
            .unwrap_err()
            .contains("rank 1")
    );

    let mut changed_question = snapshot(vec![question(vec![candidate("a", 1)])]);
    changed_question.generation = 2;
    changed_question.questions[0].question.push('!');
    assert!(
        compare_rankings(&before, &changed_question)
            .unwrap_err()
            .contains("question input")
    );
}
