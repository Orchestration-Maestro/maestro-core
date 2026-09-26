//! What the document records' refusals say, and the store's refusals they
//! carry unchanged.

use crate::{document::Error, store};
use std::error;

#[test]
fn every_refusal_says_what_went_wrong() {
    let document = Error::DocumentConflict("doc-a".to_owned()).to_string();
    let revision = Error::RevisionConflict("rev-a".to_owned()).to_string();
    assert!(document.contains("doc-a"), "{document}");
    assert!(revision.contains("rev-a"), "{revision}");
    assert_ne!(
        document.replace("doc-a", ""),
        revision.replace("rev-a", ""),
        "each conflict says what it is about"
    );
    for error in [
        Error::DocumentConflict(String::new()),
        Error::RevisionConflict(String::new()),
    ] {
        assert!(error::Error::source(&error).is_none(), "{error}");
    }
}

#[test]
fn a_store_refusal_is_carried_with_its_message_and_source() {
    let inner = || store::Error::Sqlite(rusqlite::Error::InvalidQuery);
    let wrapped = Error::from(inner());
    assert!(
        matches!(&wrapped, Error::Store(store::Error::Sqlite(_))),
        "{wrapped:?}"
    );
    assert_eq!(wrapped.to_string(), inner().to_string());
    let reason = error::Error::source(&wrapped).map(ToString::to_string);
    assert_eq!(reason, Some(rusqlite::Error::InvalidQuery.to_string()));
    let from_sqlite = Error::from(rusqlite::Error::InvalidQuery);
    assert!(
        matches!(
            from_sqlite,
            Error::Store(store::Error::Sqlite(rusqlite::Error::InvalidQuery))
        ),
        "{from_sqlite:?}"
    );
}
