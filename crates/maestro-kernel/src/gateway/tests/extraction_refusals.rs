//! Unlisted job and event kinds cannot become extraction candidates, even
//! alongside valid documentary claims in the same completion.

use super::{
    extract::{completion, extractor_card, props},
    fixture::BUILD,
    stub::{StubRouter, answer},
};
use crate::gateway::{
    Candidate, Error, ExtractRequest, ModelCard, ModelPort, RouterClient, fake::FakeModels,
};
use serde_json::{Value, json};
use std::{fs, slice};

/// A source-backed documentary relation within the closed vocabulary.
fn documentary_candidate() -> Value {
    json!({
        "subject": {"kind": "Component", "name": "scheduler"},
        "predicate": "DEPENDS_ON",
        "object": {"type": "entity", "kind": "Component", "name": "database"},
        "quote": "The scheduler depends on the database."
    })
}

/// Sends crafted candidate JSON through the real constrained router decoder.
async fn router_extract(
    card: &ModelCard,
    request: &ExtractRequest,
    candidates: &[Value],
) -> Result<Vec<Candidate>, Error> {
    let content = json!({"candidates": candidates}).to_string();
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props(BUILD)),
        (
            "/models/answer/v1/chat/completions",
            answer(200, &completion(&content)),
        ),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    client.extract(card, request).await
}

/// An error, not a filtered or partial success, must replace the entire output.
async fn refuse_endpoint_kinds(cases: &[(&str, &str)]) {
    let (path, card) = extractor_card();
    let request = ExtractRequest::new(
        "The scheduler depends on the database. A scheduled-run waits for event satisfaction.",
    )
    .unwrap();
    assert!(
        FakeModels
            .extract(&card, &request)
            .await
            .unwrap()
            .is_empty()
    );
    let valid = documentary_candidate();
    assert_eq!(
        router_extract(&card, &request, slice::from_ref(&valid))
            .await
            .unwrap()
            .len(),
        1
    );
    for (subject, object) in cases {
        let refused = json!({
            "subject": {"kind": subject, "name": "scheduled-run"},
            "predicate": "DEPENDS_ON",
            "object": {"type": "entity", "kind": object, "name": "documented-type"},
            "quote": "A scheduled-run waits for event satisfaction."
        });
        for candidates in [
            [valid.clone(), refused.clone()],
            [refused.clone(), valid.clone()],
        ] {
            let result = router_extract(&card, &request, &candidates).await;
            assert!(
                matches!(&result, Err(Error::InvalidAnswer { reason })
                    if reason == "entity kind is outside the closed vocabulary"),
                "{subject} -> {object} must refuse the whole output: {result:?}"
            );
        }
    }
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn constrained_gateway_refuses_instance_job_kinds_without_partial_candidates() {
    refuse_endpoint_kinds(&[
        ("Job", "Component"),
        ("Component", "Job"),
        ("JobType", "Component"),
        ("Component", "JobType"),
    ])
    .await;
}

#[tokio::test]
async fn constrained_gateway_refuses_event_dependency_kinds_without_partial_candidates() {
    refuse_endpoint_kinds(&[
        ("Event", "Component"),
        ("Component", "Event"),
        ("Concept", "JobType"),
        ("JobType", "Concept"),
        ("Event", "JobType"),
        ("JobType", "Event"),
    ])
    .await;
}
