//! Every answer and refusal keeps, for evaluation only, the anchors of the
//! bundle it answered from.

use super::*;
use crate::search::evidence::Anchor;

const PASSAGE: &str = "The local service uses verified instructions.";

/// The anchors of the tests' one-passage bundle.
fn expected_anchors() -> Vec<Anchor> {
    vec![Anchor {
        source_ref: "https://example.org/docs".to_owned(),
        doc_id: "document-1".to_owned(),
        revision_id: "revision-1".to_owned(),
        section_id: Some("section-1".to_owned()),
        span: [0, PASSAGE.len()],
        digest: format!("sha256:{}", Digest::of(PASSAGE.as_bytes()).as_str()),
    }]
}

/// The answer to `reply` over the tests' one-passage bundle, planned with
/// `min_rerank_score` against a top reranker score of 0.
async fn answer_to(reply: &str, min_rerank_score: Option<f32>) -> super::super::Answer {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("How does it work?");
    let port = ScriptedPort::new(&[reply]);
    let prompt = AnswerPrompt::Version(PromptVersion::V1);
    let plan = AnswerPlan {
        min_rerank_score,
        top_rerank_score: Some(0.0),
        answer_prompt: &prompt,
    };
    answer_relevant(
        &port,
        &request,
        Some(&answerer),
        bundle(&request.question, "en", PASSAGE),
        plan,
    )
    .await
    .expect("answer or refusal")
}

#[test]
fn anchors_copy_each_passage_s_source_identity_and_digest() {
    let bundle = bundle("How does it work?", "en", PASSAGE);

    let anchors: Vec<Anchor> = bundle.passages.iter().map(Anchor::from).collect();
    assert_eq!(anchors, expected_anchors());
    let mut whole = expected_anchors();
    whole[0].section_id = None;
    assert!(
        !serde_json::to_string(&whole[0])
            .unwrap()
            .contains("section_id")
    );
}

#[tokio::test]
async fn a_delivered_answer_keeps_the_anchors_of_its_bundle() {
    let answer = answer_to(&format!("{PASSAGE} [1]"), None).await;

    assert!(answer.refusal.is_none());
    assert_eq!(answer.delivered, expected_anchors());
    assert!(
        !serde_json::to_string(&answer)
            .unwrap()
            .contains("revision-1")
    );
}

#[tokio::test]
async fn a_refusal_keeps_the_anchors_of_its_bundle() {
    let not_found = answer_to("NOT_FOUND", None).await;
    let below = answer_to("unused", Some(0.5)).await;

    assert_eq!(
        not_found.refusal.map(|refusal| refusal.code),
        Some(RefusalCode::NotFound)
    );
    assert_eq!(not_found.delivered, expected_anchors());
    assert_eq!(
        below.refusal.map(|refusal| refusal.code),
        Some(RefusalCode::NoEvidence)
    );
    assert_eq!(below.delivered, expected_anchors());
}
