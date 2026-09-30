//! Model-registry and card-registration error text and source chaining.

use super::support::Scratch;
use crate::{
    artifact::Digest,
    gateway::card_types::CardError,
    model::{Error as ModelError, ModelCardRegistrationError, ModelCardRegistrationOutcome},
    store,
};
use std::{error::Error as _, io};

#[test]
fn model_errors_have_stable_display_text_and_source_chains() {
    for (error, expected, has_source) in [
        (
            ModelError::Unauthorized,
            "the collection is not writable in this scope",
            false,
        ),
        (
            ModelError::Invalid("blank run_id".to_owned()),
            "invalid model-registry record: blank run_id",
            false,
        ),
        (
            ModelError::Integrity("bad row".to_owned()),
            "model-registry integrity check failed: bad row",
            false,
        ),
        (
            ModelError::Card(CardError::Invalid("bad schema".to_owned())),
            "not a valid maestro-model-card/1 or maestro-model-card/2 model card: bad schema",
            true,
        ),
        (
            ModelError::Store(store::Error::Sqlite(rusqlite::Error::InvalidQuery)),
            "the kernel database refused the operation",
            true,
        ),
    ] {
        assert_eq!(error.to_string(), expected);
        assert_eq!(error.source().is_some(), has_source);
    }

    let scratch = Scratch::new();
    let artifact_error = scratch.store().get(&Digest::of(b"missing")).unwrap_err();
    let error = ModelError::Card(CardError::Store(artifact_error));
    assert_eq!(
        error.to_string(),
        "the model card could not be stored or read"
    );
    assert!(error.source().is_some());
}

#[test]
fn registration_errors_chain_the_error_they_wrap() {
    let wrapped: [(ModelCardRegistrationError, &str); 3] = [
        (
            ModelCardRegistrationError::Model(ModelError::Invalid("blank run_id".to_owned())),
            "invalid model-registry record: blank run_id",
        ),
        (
            ModelCardRegistrationError::Store(store::Error::Sqlite(rusqlite::Error::InvalidQuery)),
            "the kernel database refused the operation",
        ),
        (
            ModelCardRegistrationError::Io(io::Error::other("disk gone")),
            "disk gone",
        ),
    ];
    for (error, expected) in wrapped {
        assert_eq!(error.to_string(), expected);
        assert_eq!(
            error.source().map(ToString::to_string).as_deref(),
            Some(expected)
        );
    }
    for error in [
        ModelCardRegistrationError::Unauthorized,
        ModelCardRegistrationError::Invalid("bad card".to_owned()),
        ModelCardRegistrationError::Integrity("bad digest".to_owned()),
    ] {
        assert!(error.source().is_none(), "{error}");
    }
}

#[test]
fn registration_outcomes_name_what_happened() {
    assert_eq!(
        ModelCardRegistrationOutcome::Recorded.to_string(),
        "recorded"
    );
    assert_eq!(
        ModelCardRegistrationOutcome::AlreadyPresent.to_string(),
        "already present"
    );
}
