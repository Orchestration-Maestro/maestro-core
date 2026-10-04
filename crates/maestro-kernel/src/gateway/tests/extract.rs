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
use crate::{
    artifact::Digest,
    facts::{EntityName as FactEntityName, Literal, LiteralKind as FactLiteralKind, Object},
    vocabulary::{EntityKind, Predicate},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, num::NonZeroU32, path::PathBuf};

pub(super) fn extractor_card() -> (PathBuf, ModelCard) {
    extractor_card_with_settings(
        128,
        Capability::Supported(BTreeMap::from([(
            "enable_thinking".to_owned(),
            ControlValue::Boolean(false),
        )])),
    )
}

fn extractor_card_with_settings(
    output_tokens: u32,
    reasoning: Capability<BTreeMap<String, ControlValue>>,
) -> (PathBuf, ModelCard) {
    let (path, store) = scratch_store();
    let mut identity = answerer_identity();
    identity.role = Role::Extractor;
    identity.router_entry = RouterEntry::parse("answer").unwrap();
    identity.invocation.limits.output_tokens = NonZeroU32::new(output_tokens);
    identity.invocation.llama_cpp_build = BUILD.to_owned();
    identity.invocation.reasoning = reasoning;
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
    assert!(matches!(
        ExtractRequest::new(" "),
        Err(super::super::Error::InvalidRequest { .. })
    ));
}

#[test]
fn extraction_decode_reuses_claim_types_and_rejects_invalid_lexemes() {
    let mut candidates = vec![json!({
        "subject": {"kind": "Parameter", "name": "mode"},
        "predicate": "REQUIRES",
        "object": {"type": "entity", "kind": "Command", "name": "run"},
        "quote": "Parameter mode requires command run"
    })];
    for (kind, value) in [
        ("text", "safe"),
        ("boolean", "true"),
        ("integer", "12"),
        ("decimal", "1.25"),
    ] {
        candidates.push(json!({
            "subject": {"kind": "Parameter", "name": "mode"},
            "predicate": "DEFAULTS_TO",
            "object": {"type": "literal", "kind": kind, "value": value},
            "quote": "The default value is present in this source."
        }));
    }
    let content = json!({"candidates": candidates}).to_string();
    let decoded = decode(&content).expect("valid entity and literals");
    assert_eq!(decoded.len(), 5);
    assert_eq!(decoded[0].quote, "Parameter mode requires command run");
    assert!(matches!(
        decoded[0].object,
        Object::Entity(FactEntityName {
            kind: EntityKind::Command,
            ..
        })
    ));
    for (candidate, kind) in decoded[1..].iter().zip([
        FactLiteralKind::Text,
        FactLiteralKind::Boolean,
        FactLiteralKind::Integer,
        FactLiteralKind::Decimal,
    ]) {
        assert!(matches!(
            &candidate.object,
            Object::Literal(Literal { kind: actual, .. }) if *actual == kind
        ));
    }

    let invalid_integer = json!({"candidates": [{
        "subject": {"kind": "Parameter", "name": "mode"},
        "predicate": "DEFAULTS_TO",
        "object": {"type": "literal", "kind": "integer", "value": "abc"}
    }]})
    .to_string();
    assert!(matches!(
        decode(&invalid_integer),
        Err(super::super::Error::InvalidAnswer { .. })
    ));
}

#[test]
fn extraction_decode_refuses_alias_duplicate_unknown_and_mismatched_shapes() {
    for content in [
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode"},"#,
            r#""predicate":"ALIAS_OF","object":{"type":"entity","kind":"Parameter","#,
            r#""name":"alias"}}]}"#,
        ),
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode"},"#,
            r#""predicate":"DEFAULTS_TO","predicate":"REQUIRES","object":{"type":"literal","#,
            r#""kind":"text","value":"safe"}}]}"#,
        ),
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode","extra":true},"#,
            r#""predicate":"DEFAULTS_TO","object":{"type":"literal","kind":"text","#,
            r#""value":"safe"}}]}"#,
        ),
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode"},"#,
            r#""predicate":"DEFAULTS_TO","object":{"type":"entity","kind":"Parameter","#,
            r#""name":"safe"}}]}"#,
        ),
        concat!(
            r#"{"candidates":[{"subject":{"kind":"Parameter","name":"mode"},"#,
            r#""predicate":"REQUIRES","object":{"type":"entity","kind":"Command","#,
            r#""name":"run"}}]}"#,
        ),
    ] {
        assert!(matches!(
            decode(content),
            Err(super::super::Error::InvalidAnswer { .. })
        ));
    }
}

pub(super) fn completion(content: &str) -> Value {
    completion_with_reason(content, "stop")
}

fn completion_with_reason(content: &str, finish_reason: &str) -> Value {
    json!({
        "choices": [{
            "index": 0,
            "finish_reason": finish_reason,
            "message": {"role": "assistant", "content": content}
        }]
    })
}

pub(super) fn props(build: &str) -> Reply {
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

/// The closed schema the extractor must send: claimable predicates only and
/// every shared entity and literal kind.
fn expected_response_format() -> Value {
    let kinds: Vec<_> = EntityKind::ALL.map(EntityKind::as_str).into();
    let predicates: Vec<_> = Predicate::ALL
        .into_iter()
        .filter(|predicate| predicate.is_claimable())
        .map(Predicate::as_str)
        .collect();
    let literal_kinds = [
        FactLiteralKind::Text,
        FactLiteralKind::Boolean,
        FactLiteralKind::Integer,
        FactLiteralKind::Decimal,
    ]
    .map(FactLiteralKind::as_str);
    let entity_name = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["kind", "name"],
        "properties": {
            "kind": {"type": "string", "enum": kinds},
            "name": {"type": "string", "minLength": 1}
        }
    });
    let candidate = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["subject", "predicate", "object", "quote"],
        "properties": {
            "subject": entity_name,
            "predicate": {"type": "string", "enum": predicates},
            "object": {"oneOf": [
                {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["type", "kind", "name"],
                    "properties": {
                        "type": {"const": "entity"},
                        "kind": {"type": "string", "enum": kinds},
                        "name": {"type": "string", "minLength": 1}
                    }
                },
                {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["type", "kind", "value"],
                    "properties": {
                        "type": {"const": "literal"},
                        "kind": {"type": "string", "enum": literal_kinds},
                        "value": {"type": "string"}
                    }
                }
            ]},
            "quote": {"type": "string", "minLength": 1}
        }
    });
    json!({
        "type": "json_schema",
        "json_schema": {
            "name": "maestro_extraction_candidates",
            "strict": true,
            "schema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["candidates"],
                "properties": {"candidates": {"type": "array", "items": candidate}}
            }
        }
    })
}

#[tokio::test]
async fn router_posts_closed_schema_pinned_settings_and_free_room() {
    let (path, card) = extractor_card_with_settings(
        4096,
        Capability::Supported(BTreeMap::from([(
            "enable_thinking".to_owned(),
            ControlValue::Boolean(false),
        )])),
    );
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
    assert_eq!(requests[1].body["max_tokens"], 1024);
    assert_eq!(requests[1].body["stream"], false);
    assert_eq!(requests[1].body["temperature"], 0.1);
    assert_eq!(
        requests[1].body["chat_template_kwargs"]["enable_thinking"],
        false
    );
    assert_eq!(
        requests[1].body["response_format"],
        expected_response_format()
    );
    let system = requests[1].body["messages"][0]["content"]
        .as_str()
        .expect("system prompt");
    assert_eq!(requests[1].body["messages"][0]["role"], "system");
    assert!(system.contains("Preserve negation"));
    assert!(system.contains("hypothetical"));
    assert!(system.contains("Quote exact supporting text"));
    assert!(
        EntityKind::ALL
            .iter()
            .all(|kind| system.contains(kind.as_str()))
    );
    assert!(
        Predicate::ALL
            .into_iter()
            .filter(|predicate| predicate.is_claimable())
            .all(|predicate| system.contains(predicate.as_str()))
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
async fn router_refuses_invalid_duplicate_and_unknown_json_with_typed_errors() {
    let (path, card) = extractor_card();
    for content in [
        r#"{"candidates":["#,
        r#"{"candidates":[],"candidates":[]}"#,
        r#"{"candidates":[],"authority":"model"}"#,
    ] {
        let stub = StubRouter::serve(vec![
            ("/models/answer/props", props(BUILD)),
            (
                "/models/answer/v1/chat/completions",
                answer(200, &completion(content)),
            ),
        ]);
        let client = RouterClient::new(stub.base()).expect("router");
        assert!(matches!(
            client
                .extract(&card, &ExtractRequest::new("source").unwrap())
                .await,
            Err(super::super::Error::InvalidAnswer { .. })
        ));
    }
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn router_accepts_exact_content_bound_and_refuses_one_byte_over() {
    let (path, card) = extractor_card();
    let at_bound = format!("{:<16384}", r#"{"candidates":[]}"#);
    assert_eq!(at_bound.len(), 16_384);
    let over_bound = format!("{at_bound} ");
    for (content, accepted) in [(&at_bound, true), (&over_bound, false)] {
        let stub = StubRouter::serve(vec![
            ("/models/answer/props", props(BUILD)),
            (
                "/models/answer/v1/chat/completions",
                answer(200, &completion(content)),
            ),
        ]);
        let client = RouterClient::new(stub.base()).expect("router");
        let result = client
            .extract(&card, &ExtractRequest::new("source").unwrap())
            .await;
        if accepted {
            assert!(result.expect("exact limit is accepted").is_empty());
        } else {
            assert!(matches!(
                result,
                Err(super::super::Error::InvalidAnswer { .. })
            ));
        }
    }
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn router_refuses_truncated_completion_with_typed_error() {
    let (path, card) = extractor_card();
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props(BUILD)),
        (
            "/models/answer/v1/chat/completions",
            answer(
                200,
                &completion_with_reason(r#"{"candidates":[]}"#, "length"),
            ),
        ),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    assert!(matches!(
        client
            .extract(&card, &ExtractRequest::new("source").unwrap())
            .await,
        Err(super::super::Error::InvalidAnswer { .. })
    ));
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn extractor_card_with_not_applicable_reasoning_is_a_typed_refusal() {
    let (path, card) = extractor_card_with_settings(128, Capability::NotApplicable);
    let stub = StubRouter::serve(vec![]);
    let client = RouterClient::new(stub.base()).expect("router");
    let error = client
        .extract(&card, &ExtractRequest::new("source").unwrap())
        .await
        .expect_err("card lacks extractor template control state");
    assert!(matches!(error, super::super::Error::InvalidRequest { .. }));
    assert!(error.to_string().contains("extractor card"));
    assert!(stub.requests().is_empty());
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn extractor_card_with_unsupported_reasoning_sends_no_template_kwargs() {
    let (path, card) = extractor_card_with_settings(128, Capability::Unsupported);
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props(BUILD)),
        (
            "/models/answer/v1/chat/completions",
            answer(200, &completion(r#"{"candidates":[]}"#)),
        ),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    assert!(
        client
            .extract(&card, &ExtractRequest::new("source").unwrap())
            .await
            .expect("unsupported reasoning has no control kwargs")
            .is_empty()
    );
    assert_eq!(stub.requests()[1].body["chat_template_kwargs"], json!({}));
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[test]
fn extraction_json_error_hides_model_written_keys_and_values() {
    let error =
        decode(r#"{"candidates":[],"private_key":"private-value"}"#).expect_err("unknown field");
    assert!(
        matches!(
            &error,
            super::super::Error::InvalidAnswer { reason }
                if reason == "invalid constrained extraction JSON: Data error at line 1 column 30"
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn extraction_refuses_answerer_cards_before_a_call() {
    let card = card(Role::Answerer);
    let result = FakeModels
        .extract(&card, &ExtractRequest::new("input").unwrap())
        .await;
    assert!(matches!(result, Err(super::super::Error::WrongRole { .. })));
}
