//! The reply cap of a chat request: the smallest of the caller's cap, the
//! answerer card's declared output limit and the kernel's ceiling.

use super::{
    super::{
        ChatRequest, DEFAULT_CHAT_OUTPUT_TOKENS, MAX_CHAT_OUTPUT_TOKENS, Message, ModelCard, Role,
        Speaker, reply_cap,
    },
    fixture::{card_of, fields},
};
use std::num::NonZeroU32;

/// An answerer card declaring `limit` output tokens, or none.
fn answerer(limit: Option<u32>) -> ModelCard {
    let mut fields = fields(Role::Answerer);
    fields.limits.output_tokens = limit.and_then(NonZeroU32::new);
    card_of(&fields)
}

/// A request of one user message, capped to `tokens`.
fn capped(tokens: u32) -> ChatRequest {
    ChatRequest::new(
        vec![Message {
            speaker: Speaker::User,
            content: "Use the evidence.".to_owned(),
        }],
        tokens,
    )
}

#[test]
fn a_card_declaring_2048_gets_2048() {
    assert_eq!(reply_cap(&answerer(Some(2048)), None), 2048);
}

#[test]
fn a_card_with_no_declared_limit_gets_the_1024_default() {
    assert_eq!(DEFAULT_CHAT_OUTPUT_TOKENS, 1024);
    assert_eq!(reply_cap(&answerer(None), None), 1024);
}

#[test]
fn an_explicit_smaller_request_wins() {
    assert_eq!(reply_cap(&answerer(Some(2048)), Some(512)), 512);
    assert_eq!(reply_cap(&answerer(None), Some(512)), 512);
    assert_eq!(reply_cap(&answerer(None), Some(1500)), 1500);
}

#[test]
fn the_ceiling_clamps_a_card_or_a_request_above_it() {
    assert_eq!(MAX_CHAT_OUTPUT_TOKENS, 2048);
    assert_eq!(reply_cap(&answerer(Some(4096)), None), 2048);
    assert_eq!(reply_cap(&answerer(Some(4096)), Some(3000)), 2048);
    assert_eq!(reply_cap(&answerer(None), Some(3000)), 2048);
}

#[test]
fn the_card_limit_clamps_a_larger_request() {
    assert_eq!(reply_cap(&answerer(Some(1000)), Some(1500)), 1000);
}

#[test]
fn the_gateway_admits_the_ceiling_and_refuses_one_token_more() {
    let card = answerer(Some(MAX_CHAT_OUTPUT_TOKENS));
    assert!(capped(MAX_CHAT_OUTPUT_TOKENS).validate(&card).is_ok());
    assert!(capped(MAX_CHAT_OUTPUT_TOKENS + 1).validate(&card).is_err());
    assert!(
        capped(MAX_CHAT_OUTPUT_TOKENS)
            .validate(&answerer(None))
            .is_ok()
    );
}
