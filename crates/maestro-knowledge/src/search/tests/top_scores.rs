//! The top reranker and fused scores of a ranked list.

use super::rerank::candidate;
use crate::search::{Ranked, top_fused_score, top_rerank_score};

fn ranked(id: &str, fusion_score: f64, score: Option<f64>) -> Ranked {
    Ranked {
        candidate: candidate(id, fusion_score, id),
        score,
    }
}

#[test]
fn the_top_rerank_score_is_the_first_ranked_candidates() {
    let ranked = [
        ranked("a", 0.01, Some(0.8)),
        ranked("b", 0.03, Some(0.2)),
        ranked("c", 0.02, None),
    ];

    assert_eq!(top_rerank_score(&ranked), Some(0.8));
}

#[test]
fn no_rerank_score_means_rerank_did_not_run() {
    assert_eq!(top_rerank_score(&[ranked("a", 0.01, None)]), None);
    assert_eq!(top_rerank_score(&[]), None);
}

#[test]
fn the_top_fused_score_is_the_highest_in_any_order() {
    let ranked = [
        ranked("a", 0.01, Some(0.8)),
        ranked("b", 0.03, Some(0.2)),
        ranked("c", 0.02, None),
    ];

    assert_eq!(top_fused_score(&ranked), Some(0.03));
    assert_eq!(top_fused_score(&[]), None);
}
