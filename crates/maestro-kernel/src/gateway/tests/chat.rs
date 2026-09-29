//! Chat sends one bounded, non-streaming prompt and refuses unusable replies.

use super::super::{
    card_v2::{Capability, ControlValue, Sampling, Template},
    port::chat_sampling,
};
use super::{
    super::{
        ChatRequest, Error, MAX_CHAT_OUTPUT_TOKENS, Message, ModelCard, ModelPort, Role, Room,
        RouterClient, RouterEntry, Speaker,
    },
    fixture::{BUILD, TEMPLATE, card},
    stub::{Reply, StubRouter, answer, free},
    v2::{answerer_identity, scratch_store},
};
use crate::artifact::Digest;
use serde_json::{Value, json};
use std::{fs, num::NonZeroU32, path::PathBuf};

/// An explicit small request for synthetic cards.
fn request(messages: &[Message]) -> ChatRequest {
    let mut request = ChatRequest::new(messages.to_vec(), 400);
    request
        .chat_template_kwargs
        .insert("enable_thinking".to_owned(), ControlValue::Boolean(false));
    request
}

/// A v2 answerer card that matches the stub and records its sampling.
fn v2_answerer_card(seed: Option<u64>) -> (PathBuf, ModelCard) {
    let (path, store) = scratch_store();
    let mut identity = answerer_identity();
    identity.invocation.limits.output_tokens = NonZeroU32::new(400);
    identity.router_entry = RouterEntry::parse("answer").expect("router entry");
    identity.invocation.llama_cpp_build = BUILD.to_owned();
    identity.formats.template = Template::Digest(Digest::of(TEMPLATE.as_bytes()));
    let Sampling::Configured(parameters) = &mut identity.invocation.sampling else {
        panic!("answerer sampling should be configured");
    };
    parameters.seed = seed;
    let card = ModelCard::record_v2(&store, &identity).expect("answerer card");
    (path, card)
}

/// A valid OpenAI-compatible chat completion.
fn completion(content: &str) -> Value {
    json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 1,
        "model": "answer",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {"role": "assistant", "content": content}
        }],
        "usage": {"prompt_tokens": 11, "completion_tokens": 4, "total_tokens": 15}
    })
}

/// A successful `/props` response for the synthetic answerer card.
fn props() -> Reply {
    answer(
        200,
        &json!({
            "default_generation_settings": {"n_ctx": 8192, "params": {}},
            "total_slots": 1,
            "model_path": "models/synthetic.gguf",
            "chat_template": TEMPLATE,
            "chat_template_caps": {},
            "modalities": {"vision": false},
            "build_info": BUILD,
            "is_sleeping": false
        }),
    )
}

fn user(content: &str) -> Message {
    Message {
        speaker: Speaker::User,
        content: content.to_owned(),
    }
}

#[tokio::test]
async fn chat_posts_the_prompt_output_limit_and_non_streaming_setting_in_free_room() {
    let messages = [user("Use the evidence.")];
    let (path, card) = v2_answerer_card(Some(1));
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props()),
        (
            "/models/answer/v1/chat/completions",
            answer(200, &completion("The source says yes [1].")),
        ),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    let answer = client
        .chat(&card, Room::Free, &ChatRequest::new(messages.to_vec(), 400))
        .await
        .expect("completion");
    assert_eq!(answer, "The source says yes [1].");
    assert_eq!(
        stub.requests(),
        [
            free("GET", "/models/answer/props", Value::Null),
            free(
                "POST",
                "/models/answer/v1/chat/completions",
                json!({
                    "messages": [{"role": "user", "content": "Use the evidence."}],
                    "max_tokens": 400,
                    "temperature": 0.1,
                    "top_p": 0.9,
                    "top_k": 40,
                    "min_p": 0.0,
                    "typical_p": 1.0,
                    "repeat_penalty": 1.0,
                    "frequency_penalty": 0.0,
                    "presence_penalty": 0.0,
                    "seed": 1,
                    "stream": false,
                    "chat_template_kwargs": {}
                })
            ),
        ]
    );
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn v1_answerer_sends_no_sampling_fields() {
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props()),
        (
            "/models/answer/v1/chat/completions",
            answer(200, &completion("The source says yes [1].")),
        ),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    client
        .chat(
            &card(Role::Answerer),
            Room::Free,
            &ChatRequest::new(vec![user("Use the evidence.")], 400),
        )
        .await
        .expect("completion");
    assert_eq!(
        stub.requests()[1].body,
        json!({
            "messages": [{"role": "user", "content": "Use the evidence."}],
            "max_tokens": 400,
            "stream": false,
            "chat_template_kwargs": {}
        })
    );
}

#[tokio::test]
async fn v2_answerer_without_a_seed_omits_the_seed_field() {
    let (path, card) = v2_answerer_card(None);
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props()),
        (
            "/models/answer/v1/chat/completions",
            answer(200, &completion("The source says yes [1].")),
        ),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    client
        .chat(
            &card,
            Room::Free,
            &ChatRequest::new(vec![user("Use the evidence.")], 400),
        )
        .await
        .expect("completion");
    let requests = stub.requests();
    let body = &requests.get(1).expect("chat request").body;
    assert_eq!(body["temperature"], json!(0.1));
    assert_eq!(body["top_p"], json!(0.9));
    assert!(body.get("seed").is_none());
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[test]
fn v2_answerer_with_not_applicable_sampling_is_refused() {
    assert!(matches!(
        chat_sampling(Some(&Sampling::NotApplicable)),
        Err(Error::InvalidRequest { .. })
    ));
    assert!(chat_sampling(None).unwrap().is_none());
}

#[test]
fn chat_template_controls_must_match_the_v2_answerer_card() {
    let (path, store) = scratch_store();
    let mut identity = answerer_identity();
    identity.invocation.limits.output_tokens = NonZeroU32::new(400);
    identity.invocation.reasoning = Capability::Supported(
        [("enable_thinking".to_owned(), ControlValue::Boolean(false))].into(),
    );
    let card = ModelCard::record_v2(&store, &identity).expect("answerer card");
    let supported = request(&[user("Use the evidence.")]);
    assert!(supported.validate(&card).is_ok());

    let mut above_card_limit = supported.clone();
    above_card_limit.max_output_tokens = 401;
    assert!(matches!(
        above_card_limit.validate(&card),
        Err(Error::InvalidRequest { .. })
    ));

    let unsupported = ChatRequest::new(vec![user("Use the evidence.")], 400);
    assert!(matches!(
        unsupported.validate(&card),
        Err(Error::InvalidRequest { .. })
    ));
    let mut enabled = supported;
    enabled
        .chat_template_kwargs
        .insert("enable_thinking".to_owned(), ControlValue::Boolean(true));
    assert!(matches!(
        enabled.validate(&card),
        Err(Error::InvalidRequest { .. })
    ));
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[test]
fn chat_template_controls_need_names_and_finite_numbers() {
    let card = card(Role::Answerer);
    for (name, value) in [
        ("", ControlValue::Boolean(false)),
        ("number", ControlValue::Number(f64::NAN)),
    ] {
        let mut request = ChatRequest::new(vec![user("Use the evidence.")], 400);
        request.chat_template_kwargs.insert(name.to_owned(), value);
        assert!(matches!(
            request.validate(&card),
            Err(Error::InvalidRequest { .. })
        ));
    }
}

#[tokio::test]
async fn chat_serializes_card_template_control_values() {
    let (path, store) = scratch_store();
    let mut identity = answerer_identity();
    identity.invocation.limits.output_tokens = NonZeroU32::new(400);
    identity.router_entry = RouterEntry::parse("answer").expect("router entry");
    identity.invocation.llama_cpp_build = BUILD.to_owned();
    identity.formats.template = Template::Digest(Digest::of(TEMPLATE.as_bytes()));
    identity.invocation.reasoning = Capability::Supported(
        [("enable_thinking".to_owned(), ControlValue::Boolean(false))].into(),
    );
    let card = ModelCard::record_v2(&store, &identity).expect("answerer card");
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props()),
        (
            "/models/answer/v1/chat/completions",
            answer(200, &completion("The source says yes [1].")),
        ),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    client
        .chat(&card, Room::Free, &request(&[user("Use the evidence.")]))
        .await
        .expect("completion");

    let requests = stub.requests();
    assert_eq!(
        requests[1].body["chat_template_kwargs"]["enable_thinking"],
        json!(false)
    );
    fs::remove_dir_all(path).expect("remove scratch card");
}

#[tokio::test]
async fn malformed_or_unexpected_completion_shapes_are_rejected() {
    let mut length = completion("partial");
    length["choices"][0]["finish_reason"] = json!("length");
    let mut missing = completion("unused");
    missing["choices"][0]["message"]["content"] = Value::Null;
    let mut multiple = completion("first");
    multiple["choices"]
        .as_array_mut()
        .expect("choices")
        .push(json!({
            "index": 1,
            "finish_reason": "stop",
            "message": {"role": "assistant", "content": "unexpected"}
        }));
    let mut tools = completion("unused");
    tools["choices"][0]["message"]["tool_calls"] = json!([{"id": "tool-1"}]);
    let mut functions = completion("unused");
    functions["choices"][0]["message"]["function_call"] = json!({"name": "execute"});
    let mut wrong_role = completion("unexpected");
    wrong_role["choices"][0]["message"]["role"] = json!("user");
    let mut wrong_index = completion("unexpected");
    wrong_index["choices"][0]["index"] = json!(1);
    let mut no_choices = completion("unused");
    no_choices["choices"] = json!([]);

    for body in [
        length,
        missing,
        multiple,
        tools,
        functions,
        wrong_role,
        wrong_index,
        no_choices,
    ] {
        let stub = StubRouter::serve(vec![
            ("/models/answer/props", props()),
            ("/models/answer/v1/chat/completions", answer(200, &body)),
        ]);
        let client = RouterClient::new(stub.base()).expect("router");
        let error = client
            .chat(
                &card(Role::Answerer),
                Room::Free,
                &request(&[user("Use the evidence.")]),
            )
            .await
            .expect_err("invalid completion");
        assert!(matches!(error, Error::InvalidAnswer { .. }), "{error}");
    }
}

#[tokio::test]
async fn chat_accepts_content_at_the_raw_byte_bound() {
    let body = completion(&"x".repeat(32_768));
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props()),
        ("/models/answer/v1/chat/completions", answer(200, &body)),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    let answer = client
        .chat(
            &card(Role::Answerer),
            Room::Free,
            &request(&[user("Use the evidence.")]),
        )
        .await
        .expect("content at its limit");
    assert_eq!(answer.len(), 32_768);
}

#[tokio::test]
async fn chat_accepts_a_response_body_at_the_byte_bound() {
    const BODY_LIMIT: usize = 262_144;
    let mut body = completion("The source says yes [1].");
    body["extra"] = json!("");
    let empty_extra_size = serde_json::to_vec(&body).expect("serialize response").len();
    body["extra"] = json!("x".repeat(BODY_LIMIT - empty_extra_size));
    assert_eq!(
        serde_json::to_vec(&body).expect("serialize response").len(),
        BODY_LIMIT
    );
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props()),
        ("/models/answer/v1/chat/completions", answer(200, &body)),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    let answer = client
        .chat(
            &card(Role::Answerer),
            Room::Free,
            &request(&[user("Use the evidence.")]),
        )
        .await
        .expect("response at its limit");
    assert_eq!(answer, "The source says yes [1].");
}

#[tokio::test]
async fn empty_tool_and_function_call_arrays_are_not_calls() {
    let mut body = completion("The source says yes [1].");
    body["choices"][0]["message"]["tool_calls"] = json!([]);
    body["choices"][0]["message"]["function_call"] = json!([]);
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props()),
        ("/models/answer/v1/chat/completions", answer(200, &body)),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    let answer = client
        .chat(
            &card(Role::Answerer),
            Room::Free,
            &request(&[user("Use the evidence.")]),
        )
        .await
        .expect("empty tool fields are not calls");
    assert_eq!(answer, "The source says yes [1].");
}

#[tokio::test]
async fn chat_rejects_content_over_the_raw_byte_bound() {
    let body = completion(&"x".repeat(32_769));
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props()),
        ("/models/answer/v1/chat/completions", answer(200, &body)),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    let error = client
        .chat(
            &card(Role::Answerer),
            Room::Free,
            &request(&[user("Use the evidence.")]),
        )
        .await
        .expect_err("oversized answer");
    assert!(matches!(error, Error::InvalidAnswer { .. }), "{error}");
}

#[tokio::test]
async fn chat_rejects_an_oversized_response_envelope() {
    let mut body = completion("The source says yes [1].");
    body["extra"] = json!("x".repeat(262_145));
    let stub = StubRouter::serve(vec![
        ("/models/answer/props", props()),
        ("/models/answer/v1/chat/completions", answer(200, &body)),
    ]);
    let client = RouterClient::new(stub.base()).expect("router");
    let error = client
        .chat(
            &card(Role::Answerer),
            Room::Free,
            &request(&[user("Use the evidence.")]),
        )
        .await
        .expect_err("oversized response envelope");
    assert!(matches!(error, Error::InvalidAnswer { .. }), "{error}");
}

#[tokio::test]
async fn an_output_limit_outside_the_local_bound_is_refused_before_http() {
    let stub = StubRouter::serve(Vec::new());
    let client = RouterClient::new(stub.base()).expect("router");
    let error = client
        .chat(
            &card(Role::Answerer),
            Room::Free,
            &ChatRequest::new(vec![user("Use the evidence.")], MAX_CHAT_OUTPUT_TOKENS + 1),
        )
        .await
        .expect_err("oversized generation budget");
    assert!(matches!(error, Error::InvalidRequest { .. }), "{error}");
    assert!(stub.requests().is_empty());
}
