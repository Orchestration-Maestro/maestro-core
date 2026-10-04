//! Rendering preserves generation framing and refuses unsupported adapters.
use super::{
    super::{
        ChatRequest, Error, FakeModels, Message, ModelPort, Role, Room, RouterClient, Speaker,
        card_v2::ControlValue,
    },
    fixture::{BUILD, TEMPLATE, card},
    stub::{StubRouter, answer, free},
};
use serde_json::json;

#[tokio::test]
async fn render_chat_sends_messages_controls_and_generation_prefix_in_free_room() {
    let card = card(Role::Answerer);
    let mut request = ChatRequest::new(
        vec![Message {
            speaker: Speaker::User,
            content: "private source".into(),
        }],
        100,
    );
    request
        .chat_template_kwargs
        .insert("enable_thinking".into(), ControlValue::Boolean(false));
    let stub = StubRouter::serve(vec![
        (
            "/models/answer/props",
            answer(200, &json!({"build_info":BUILD,"chat_template":TEMPLATE})),
        ),
        (
            "/models/answer/apply-template",
            answer(
                200,
                &json!({"prompt":"<bos>user: private source<assistant>"}),
            ),
        ),
        (
            "/models/answer/tokenize",
            answer(200, &json!({"tokens":[1,2,3,4,5]})),
        ),
    ]);
    let client = RouterClient::new(stub.base()).unwrap();
    let rendered = client
        .render_chat(&card, Room::Free, &request)
        .await
        .unwrap();
    assert_eq!(rendered, "<bos>user: private source<assistant>");
    assert_eq!(
        client
            .tokenize(&card, Room::Free, &rendered)
            .await
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        stub.requests(),
        [
            free("GET", "/models/answer/props", serde_json::Value::Null),
            free(
                "POST",
                "/models/answer/apply-template",
                json!({
                    "messages":[{"role":"user","content":"private source"}],
                    "add_generation_prompt":true, "chat_template_kwargs":{"enable_thinking":false},
                })
            ),
            free(
                "POST",
                "/models/answer/tokenize",
                json!({
                    "content":rendered,"add_special":true,"parse_special":true,
                })
            ),
        ]
    );
    assert!(matches!(
        FakeModels.render_chat(&card, Room::Free, &request).await,
        Err(Error::Unsupported)
    ));
}
