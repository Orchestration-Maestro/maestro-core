use super::*;
use maestro_kernel::gateway::{CardFields, Limits, MAX_CHAT_OUTPUT_TOKENS, Role, SuiteResult};
use std::num::NonZeroU32;

/// An answerer whose first-schema card declares no output limit.
fn unlimited_answerer(scratch: &Scratch) -> RegisteredAnswerer {
    let fields = CardFields {
        role: Role::Answerer,
        router_entry: RouterEntry::parse("qwen3-4b").expect("router entry"),
        file_digest: Digest::of(b"model file"),
        template_digest: Some(Digest::of(b"template")),
        server_build: "b6500-3f2c9a1b".to_owned(),
        dimensions: None,
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).expect("nonzero context"),
            output_tokens: None,
        },
        suite_results: vec![SuiteResult {
            suite: "synthetic-retrieval".to_owned(),
            report: Digest::of(b"report"),
        }],
    };
    let card = ModelCard::record(&Store::new(&scratch.0), &fields).expect("answerer card");
    RegisteredAnswerer {
        id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
        card,
    }
}

/// The cap of each chat request an ask with `output_tokens` sends to
/// `answerer`, whose first reply is rejected, and the cap its answer records.
async fn reply_caps(
    answerer: &RegisteredAnswerer,
    output_tokens: Option<u32>,
) -> (Vec<u32>, Option<u32>) {
    let mut request = request("How does the service work?");
    request.budget.output_tokens = output_tokens;
    let evidence = bundle(
        &request.question,
        "en",
        "The service uses verified instructions.",
    );
    let port = ScriptedPort::new(&["[1]", "The service uses verified instructions. [1]"]);
    let answer = answer_bundle(
        &port,
        &request,
        Some(answerer),
        evidence,
        &PromptVersion::V1.into(),
    )
    .await
    .expect("answer after one repair");
    let calls = port.chat_calls.lock().expect("chat-call lock");
    let caps = calls
        .iter()
        .map(|(_, _, chat)| chat.max_output_tokens)
        .collect();
    (caps, answer.reply_cap)
}

#[tokio::test]
async fn each_attempt_chats_under_the_smallest_of_the_request_card_and_ceiling_caps() {
    let scratch = Scratch::new();
    let cases = [
        ("a card declaring 2048", Some(2048), None, 2048),
        ("no declared limit", None, None, 1024),
        ("an explicit smaller request", Some(2048), Some(512), 512),
        (
            "a card above the ceiling",
            Some(MAX_CHAT_OUTPUT_TOKENS * 2),
            None,
            MAX_CHAT_OUTPUT_TOKENS,
        ),
    ];
    for (case, limit, output_tokens, cap) in cases {
        let answerer = limit.map_or_else(
            || unlimited_answerer(&scratch),
            |limit| scratch.answerer_with_output_limit(limit),
        );
        assert_eq!(
            reply_caps(&answerer, output_tokens).await,
            (vec![cap, cap], Some(cap)),
            "{case}"
        );
    }
}

#[tokio::test]
async fn an_ask_that_never_chats_records_no_reply_cap() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer_with_output_limit(2048);
    let request = request("How does the service work?");
    let answer = answer_bundle(
        &ScriptedPort::new(&[]),
        &request,
        Some(&answerer),
        empty_bundle(&request.question),
        &PromptVersion::V1.into(),
    )
    .await
    .expect("no-evidence refusal");

    assert_eq!(answer.reply_cap, None);
    let json = serde_json::to_value(&answer).expect("answer JSON");
    assert!(json.get("reply_cap").is_none());
}
