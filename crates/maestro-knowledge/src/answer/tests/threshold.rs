//! The reranker relevance threshold: below it, `ask` refuses without chat.

use super::*;
use crate::search::{Candidate, Fused, Ranked, top_rerank_score};
use std::collections::BTreeMap;

const PASSAGE: &str = "The local service uses verified instructions.";
const REPLY: &str = "The local service uses verified instructions. [1]";

/// Answers `question` over one passage, with `relevance`, and counts the
/// chat calls.
async fn answer_with(question: &str, relevance: AnswerPlan<'_>) -> (super::super::Answer, usize) {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request(question);
    let port = ScriptedPort::new(&[REPLY]);
    let answer = answer_relevant(
        &port,
        &request,
        Some(&answerer),
        bundle(question, "en", PASSAGE),
        relevance,
    )
    .await
    .expect("answer or refusal");
    (answer, port.calls.load(Ordering::Relaxed))
}

/// The v1 prompt every threshold case answers with.
static V1: AnswerPrompt = AnswerPrompt::Version(PromptVersion::V1);

fn relevance(min_rerank_score: Option<f32>, top_rerank_score: Option<f64>) -> AnswerPlan<'static> {
    AnswerPlan {
        min_rerank_score,
        top_rerank_score,
        answer_prompt: &V1,
    }
}

#[tokio::test]
async fn no_threshold_changes_nothing() {
    let (answer, calls) = answer_with("How does it work?", relevance(None, Some(-3.0))).await;

    assert_eq!(calls, 1);
    assert!(answer.refusal.is_none());
}

#[tokio::test]
async fn a_top_score_below_the_threshold_refuses_without_chat() {
    let (answer, calls) = answer_with("How does it work?", relevance(Some(0.5), Some(0.49))).await;

    assert_eq!(calls, 0);
    assert!(answer.citations.is_empty());
    let refusal = answer.refusal.expect("refusal");
    assert_eq!(refusal.code, RefusalCode::NoEvidence);
    assert_eq!(
        refusal.message,
        "The best passage was below the relevance threshold."
    );
}

#[tokio::test]
async fn a_french_refusal_below_the_threshold_says_so_in_french() {
    let question = "Comment fonctionne le service de planification ?";
    let (answer, calls) = answer_with(question, relevance(Some(0.5), Some(0.1))).await;

    assert_eq!(calls, 0);
    assert_eq!(answer.lang, "fr");
    assert_eq!(
        answer.refusal.expect("refusal").message,
        "Le meilleur passage est sous le seuil de pertinence."
    );
}

#[tokio::test]
async fn a_top_score_at_or_above_the_threshold_answers() {
    for top in [0.5, 0.51] {
        let (answer, calls) =
            answer_with("How does it work?", relevance(Some(0.5), Some(top))).await;

        assert_eq!(calls, 1);
        assert!(answer.refusal.is_none());
    }
}

#[tokio::test]
async fn without_a_rerank_score_the_threshold_does_nothing() {
    let (answer, calls) = answer_with("How does it work?", relevance(Some(0.5), None)).await;

    assert_eq!(calls, 1);
    assert!(answer.refusal.is_none());
}

/// A ranked candidate `id` with the reranker score `score`.
fn ranked(id: &str, score: f64) -> Ranked {
    Ranked {
        candidate: Candidate {
            header: None,
            fused: Fused {
                chunk_id: id.to_owned(),
                score: 0.0,
                ranks: BTreeMap::new(),
            },
            text: String::new(),
        },
        score: Some(score),
    }
}

#[tokio::test]
async fn a_prior_that_demotes_the_best_scored_passage_still_answers() {
    let demoted = [ranked("official", 0.5), ranked("repository", 0.9)];
    let plan = relevance(Some(0.6), top_rerank_score(&demoted));
    let (answer, calls) = answer_with("How does it work?", plan).await;

    assert_eq!(calls, 1);
    assert!(answer.refusal.is_none());
}
