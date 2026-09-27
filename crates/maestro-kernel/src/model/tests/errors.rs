//! Model-registry error text and source chaining.

use super::support::Scratch;
use crate::{artifact::Digest, gateway::card_types::CardError, model::Error as ModelError, store};
use std::error::Error as _;

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
