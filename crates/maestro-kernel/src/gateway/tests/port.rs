//! Tests of the port's refusals: each says what was refused and why.

use super::{
    super::{Error, Role},
    fixture::{FILE_DIGEST, digest},
};
use std::error::Error as _;

#[test]
fn a_role_is_named_as_a_card_writes_it() {
    let names = [Role::Embedder, Role::Reranker, Role::Answerer].map(|role| role.to_string());
    assert_eq!(names, ["embedder", "reranker", "answerer"]);
}

#[test]
fn every_refusal_names_what_it_refuses() {
    let cases = [
        (
            Error::WrongRole {
                card: digest(FILE_DIGEST),
                role: Role::Reranker,
                needed: Role::Embedder,
            },
            vec![FILE_DIGEST, "reranker", "embedder"],
        ),
        (
            Error::CardMismatch {
                card: digest(FILE_DIGEST),
                property: "build_info",
                recorded: "b6500-3f2c9a1b".to_owned(),
                reported: "b6501-0c1d2e3f".to_owned(),
            },
            vec![
                FILE_DIGEST,
                "build_info",
                "b6500-3f2c9a1b",
                "b6501-0c1d2e3f",
            ],
        ),
        (
            Error::Unavailable {
                reason: "no free room for it".to_owned(),
            },
            vec!["no free room for it"],
        ),
        (
            Error::UnknownRoom {
                message: "'spare' is no room".to_owned(),
            },
            vec!["'spare' is no room"],
        ),
        (
            Error::Refused {
                status: 404,
                code: Some("model_not_found".to_owned()),
                message: "no model called 'embed'".to_owned(),
            },
            vec!["404", "model_not_found", "no model called 'embed'"],
        ),
        (
            Error::Refused {
                status: 502,
                code: None,
                message: "Bad Gateway".to_owned(),
            },
            vec!["502", "Bad Gateway"],
        ),
        (
            Error::InvalidAnswer {
                reason: "2 vectors for 3 inputs".to_owned(),
            },
            vec!["2 vectors for 3 inputs"],
        ),
    ];
    for (error, parts) in cases {
        let message = error.to_string();
        for part in parts {
            assert!(
                message.contains(part),
                "{part:?} is missing from: {message}"
            );
        }
        assert!(error.source().is_none(), "{message}");
    }
}
