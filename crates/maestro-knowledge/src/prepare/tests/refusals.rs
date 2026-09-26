//! The refusals: what each says, and the cause each keeps.

use super::{
    super::TokenizerError,
    support::{MODEL_FILE, digest},
};
use maestro_kernel::gateway::{self, Role};
use std::{error::Error as _, io};

/// A refusal of each kind.
fn every_refusal() -> [TokenizerError; 7] {
    [
        TokenizerError::NotAnEmbedder {
            card: digest(MODEL_FILE),
            role: Role::Reranker,
        },
        TokenizerError::Unavailable {
            reason: "no free room".to_owned(),
        },
        TokenizerError::Port(gateway::Error::Refused {
            status: 500,
            code: None,
            message: "boom".to_owned(),
        }),
        TokenizerError::Disagreement {
            fixture: "nul".to_owned(),
            input: "a\0b".to_owned(),
            native: vec![0, 10, 3, 275, 2],
            router: vec![0, 10, 275, 2],
        },
        TokenizerError::Fixtures(serde_json::from_str::<u32>("x").unwrap_err()),
        TokenizerError::Start(io::Error::other("no thread left")),
        TokenizerError::Stopped,
    ]
}

#[test]
fn every_refusal_says_what_it_refuses() {
    let said: Vec<String> = every_refusal().iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        [
            "the model card \
             sha256:ca56847038f3f329524caec5a86e14865f49918d68094c90dd021a5e67b927f6 is a \
             reranker's, and chunks are counted in an embedder's tokens",
            "the embedder does not fit in the router's free room, and counting never unloads \
             another model: no free room",
            "the router did not tokenize: refused with 500: boom",
            "the router's IDs for the parity fixture nul differ from those of the native \
             counter: native [0, 10, 3, 275, 2], router [0, 10, 275, 2]",
            "the parity fixtures built into maestro-knowledge are not valid: expected value at \
             line 1 column 1",
            "the tokenizer's thread could not start: no thread left",
            "the tokenizer's thread stopped: a call to the model port panicked",
        ]
    );
}

#[test]
fn a_refusal_keeps_the_cause_it_wraps() {
    let causes: Vec<Option<String>> = every_refusal()
        .iter()
        .map(|refusal| refusal.source().map(ToString::to_string))
        .collect();
    assert_eq!(
        causes,
        [
            None,
            None,
            Some("refused with 500: boom".to_owned()),
            None,
            Some("expected value at line 1 column 1".to_owned()),
            Some("no thread left".to_owned()),
            None,
        ]
    );
}
