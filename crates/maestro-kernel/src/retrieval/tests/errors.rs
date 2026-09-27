//! Nonblank retrieval refusals and preserved store error sources.

use crate::{retrieval::Error, store};
use std::error::Error as _;

#[test]
fn every_refusal_has_a_nonblank_display() {
    for error in [
        Error::Store(store::Error::Sqlite(rusqlite::Error::InvalidQuery)),
        Error::UnknownOrInaccessible,
        Error::ProjectionMissing,
        Error::ProfileMismatch {
            expected: "identifiers/1".to_owned(),
            found: "identifiers/2".to_owned(),
        },
        Error::InvalidInput("bad input".to_owned()),
        Error::InputConflict,
        Error::MembershipConflict,
        Error::TooLarge,
        Error::Cancelled,
        Error::TimedOut,
    ] {
        assert!(!error.to_string().trim().is_empty(), "{error:?}");
    }
}

#[test]
fn store_refusal_preserves_its_error_source() {
    let error = Error::Store(store::Error::Sqlite(rusqlite::Error::InvalidQuery));
    assert!(error.source().is_some());
}
