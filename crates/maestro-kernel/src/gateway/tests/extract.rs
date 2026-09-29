//! Extraction is closed, card-bound and atomic: malformed replies admit no candidates.

use super::super::extract::decode;
use super::super::{
    ExtractRequest, ModelCard, ModelPort, Role, RouterClient, RouterEntry,
    card_v2::{Capability, ControlValue, Template},
    fake::FakeModels,
};
use super::{
    fixture::{BUILD, TEMPLATE, TEMPLATE_DIGEST, card},
    stub::{Reply, StubRouter, answer},
    v2::{answerer_identity, scratch_store},
};
use crate::artifact::Digest;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::PathBuf};

fn extractor_card() -> (PathBuf, ModelCard) {
    let (path, store) = scratch_store();
    let mut identity = answerer_identity();
    identity.role = Role::Extractor;
    identity.router_entry = RouterEntry::parse("answer").unwrap();
    identity.invocation.llama_cpp_build = BUILD.to_owned();
    identity.invocation.reasoning = Capability::Supported(BTreeMap::from([(
        "enable_thinking".to_owned(),
        ControlValue::Boolean(false),
    )]));
    identity.formats.template = Template::Digest(Digest::parse(TEMPLATE_DIGEST).unwrap());
    let card = ModelCard::record_v2(&store, &identity).expect("extractor card");
    (path, card)
}

#[tokio::test]
async fn fake_extractor_returns_no_candidates_for_unrecognized_input() {
    let (path, card) = extractor_card();
    let result = FakeModels
        .extract(&card, &ExtractRequest::new("source text").unwrap())
        .await
        .expect("fake extraction is deterministic");
    assert!(result.is_empty());
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[test]
fn extraction_request_rejects_empty_input() {
    assert!(ExtractRequest::new(" ").is_err());
}

#[test]
fn extraction_decode_accepts_only_complete_claimable_candidates() {
    let content = concat!(
        r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode"},"#,
        r#""predicate":"DEFAULTS_TO","object":{"type":"literal","kind":"Text","value":"safe"}}]}"#,
    );
    let candidates = decode(content).expect("valid candidate");
    assert_eq!(candidates.len(), 1);
    assert!(matches!(
        candidates[0].object,
        super::super::CandidateObject::Literal { .. }
    ));
    for invalid in [
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode"},"#,
            r#""predicate":"ALIAS_OF","object":{"type":"entity","kind":"Parameter", "#,
            r#""name":"alias"}}]}"#,
        ),
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode"},"#,
            r#""predicate":"DEFAULTS_TO","object":{"type":"entity","kind":"Parameter", "#,
            r#""name":"safe"}}]}"#,
        ),
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode","extra":true},"#,
            r#""predicate":"DEFAULTS_TO","object":{"type":"literal","kind":"Text", "#,
            r#""value":"safe"}}]}"#,
        ),
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode"},"#,
            r#""predicate":"DEFAULTS_TO","predicate":"REQUIRES","object":{"type":"literal", "#,
            r#""kind":"Text","value":"safe"}}]}"#,
        ),
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode"},"#,
            r#""predicate":"DEFAULTS_TO","object":{"type":"literal","kind":"Text", "#,
            r#""value":"safe"}},{"subject":{"kind":"Unknown","name":"other"},"#,
            r#""predicate":"DEFAULTS_TO","object":{"type":"literal","kind":"Text", "#,
            r#""value":"x"}}]}"#,
        ),
    ] {
        assert!(
            decode(invalid).is_err(),
            "accepted invalid output: {invalid}"
        );
    }
}

fn completion(content: &str) -> Value {
    json!({
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {"role": "assistant", "content": content}
        }]
    })
}

fn props(build: &str) -> Reply {
    answer(
        200,
        &json!({
            "default_generation_settings": {"n_ctx":8192,"params":{}},
            "total_slots":1,"model_path":"models/synthetic.gguf", "chat_template":TEMPLATE,
            "chat_template_caps":{},"modalities":{"vision":false},
            "build_info":build,"is_sleeping":false
        }),
    )
}

#[tokio::test]
async fn router_posts_closed_schema_pinned_settings_and_free_room() {
    let (path, card) = extractor_card();
    let content = r#"{"candidates":[]}"#;
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props(BUILD)),
        (
            "/models/answer/v1/chat/completions",
            answer(200, &completion(content)),
        ),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    assert!(
        client
            .extract(&card, &ExtractRequest::new("source").unwrap())
            .await
            .unwrap()
            .is_empty()
    );
    let requests = stub.requests();
    assert_eq!(requests[0].room.as_deref(), Some("free"));
    assert_eq!(requests[1].room.as_deref(), Some("free"));
    assert_eq!(requests[1].body["max_tokens"], 128);
    assert_eq!(requests[1].body["stream"], false);
    assert_eq!(requests[1].body["temperature"], 0.1);
    assert_eq!(
        requests[1].body["chat_template_kwargs"]["enable_thinking"],
        false
    );
    assert_eq!(requests[1].body["response_format"]["type"], "json_schema");
    assert!(
        requests[1].body["response_format"]["json_schema"]["schema"]["properties"]["candidates"]
            .is_object()
    );
    assert_eq!(requests[1].body["messages"][1]["content"], "source");
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn router_refuses_wrong_role_card_mismatch_and_insufficient_room_before_extraction() {
    let request = ExtractRequest::new("source").unwrap();
    let stub = StubRouter::serve(vec![]);
    let client = RouterClient::new(stub.base()).expect("router");
    assert!(matches!(
        client.extract(&card(Role::Answerer), &request).await,
        Err(super::super::Error::WrongRole { .. })
    ));
    assert!(stub.requests().is_empty());

    let (path, card) = extractor_card();
    let mismatch = StubRouter::serve(vec![("/models/answer/props", props("other-build"))]);
    let client = RouterClient::new(mismatch.base()).expect("router");
    assert!(matches!(
        client.extract(&card, &request).await,
        Err(super::super::Error::CardMismatch { .. })
    ));
    assert_eq!(mismatch.requests().len(), 1);

    let unavailable = StubRouter::serve(vec![(
        "/models/answer/props",
        answer(
            503,
            &json!({"error":{"code":"insufficient_room","message":"full"}}),
        ),
    )]);
    let client = RouterClient::new(unavailable.base()).expect("router");
    assert!(matches!(
        client.extract(&card, &request).await,
        Err(super::super::Error::Unavailable { .. })
    ));
    assert_eq!(unavailable.requests().len(), 1);
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn invalid_or_overflowing_router_output_yields_no_candidates() {
    let (path, card) = extractor_card();
    for content in [
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"x"},"#,
            r#""predicate":"DEFAULTS_TO","object":{"type":"literal","kind":"Text", "#,
            r#""value":"x"}},"#,
        ),
        &"x".repeat(16_385),
    ] {
        let stub = StubRouter::serve(vec![
            ("/models/answer/props", props(BUILD)),
            (
                "/models/answer/v1/chat/completions",
                answer(200, &completion(content)),
            ),
        ]);
        let client = RouterClient::new(stub.base()).expect("router");
        assert!(
            client
                .extract(&card, &ExtractRequest::new("source").unwrap())
                .await
                .is_err()
        );
    }
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn extraction_refuses_answerer_cards_before_a_call() {
    let card = card(Role::Answerer);
    let result = FakeModels
        .extract(&card, &ExtractRequest::new("input").unwrap())
        .await;
    assert!(matches!(result, Err(super::super::Error::WrongRole { .. })));
}
