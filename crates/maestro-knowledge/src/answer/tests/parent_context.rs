use super::{PromptVersion, RefusalCode, Scratch, ScriptedPort, answer_bundle, bundle, request};
use maestro_kernel::{artifact::Digest, evidence::Span};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn parent_only_citations_keep_exact_spans_in_answers_and_refusals() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("Which columns describe the service?");
    let mut evidence = bundle(&request.question, "en", "Service columns.");
    let mut row = evidence.passages[0].clone();
    row.n = 2;
    row.span = Span {
        start: 100,
        end: 112,
    };
    row.text = "Service row.".to_owned();
    row.digest = Digest::of(row.text.as_bytes());
    evidence.passages.push(row);
    let mut trace = evidence.trace[0].clone();
    trace.n = 2;
    evidence.trace.push(trace);
    evidence.trace[0].chunk_ids.clear();
    evidence.trace[0].parent_context_of = vec!["chunk-1".to_owned()];
    serde_json::to_vec(&evidence).unwrap();

    let answered = answer_bundle(
        &ScriptedPort::new(&["The service columns describe it. [1]"]),
        &request,
        Some(&answerer),
        evidence.clone(),
        &PromptVersion::V1.into(),
    )
    .await
    .unwrap();
    assert!(answered.refusal.is_none());
    assert_eq!(answered.citations[0].chunk_id, "chunk-1");
    assert_eq!(answered.citations[0].span, [0, "Service columns.".len()]);
    for (reply, registered, code) in [
        ("NOT_FOUND", Some(&answerer), RefusalCode::NotFound),
        ("", None, RefusalCode::AnswererUnavailable),
    ] {
        let refused = answer_bundle(
            &ScriptedPort::new(&[reply]),
            &request,
            registered,
            evidence.clone(),
            &PromptVersion::V1.into(),
        )
        .await
        .unwrap();
        assert_eq!(refused.refusal.unwrap().code, code);
        assert_eq!(refused.closest[0], answered.citations[0]);
    }
}

#[tokio::test]
async fn i5_not_found_marker_returns_not_found_with_closest_passage_metadata() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("How do I list the registered sources?");
    let passage = bundle(&request.question, "en", "The docs list model entries.");
    let port = ScriptedPort::new(&["NOT_FOUND"]);

    let answer = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        passage,
        &PromptVersion::V1.into(),
    )
    .await
    .expect("safe no-evidence refusal");

    assert_eq!(port.calls.load(Ordering::Relaxed), 1);
    assert_eq!(answer.answer, "");
    assert_eq!(answer.closest.len(), 1);
    assert_eq!(answer.closest[0].source_ref, "https://example.org/docs");
    let refusal = answer.refusal.expect("refusal");
    assert_eq!(refusal.code, RefusalCode::NotFound);
    assert_eq!(
        refusal.message,
        "The available passages do not answer the question."
    );
}
