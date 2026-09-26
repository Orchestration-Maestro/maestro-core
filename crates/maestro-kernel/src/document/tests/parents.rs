//! Collections, their sources and their documents: collections and sources
//! follow their declaration, while a document keeps the collection, source and
//! source reference it was first recorded with.

use super::support::{Scratch, collection, document, source};
use crate::{
    document::{Collection, Document, Error, Source},
    store,
};
use rusqlite::ffi;
use std::collections::BTreeMap;

/// Whether `error` is SQLite refusing a constraint, with the extended code
/// `code`.
fn refused(error: &Error, code: i32) -> bool {
    matches!(
        error,
        Error::Store(store::Error::Sqlite(rusqlite::Error::SqliteFailure(failure, _)))
            if failure.extended_code == code
    )
}

#[test]
fn a_collection_is_recorded_then_follows_its_declaration() {
    let scratch = Scratch::new();
    let database = scratch.empty();
    assert_eq!(database.collection("ctm").unwrap(), None);
    database.record_collection(&collection("ctm")).unwrap();
    let recorded = Collection {
        id: "ctm".to_owned(),
        title: "The ctm collection".to_owned(),
        visibility: "private".to_owned(),
        profiles: BTreeMap::from([("chunking".to_owned(), "structural-500-700/1".to_owned())]),
    };
    assert_eq!(database.collection("ctm").unwrap(), Some(recorded));
    let declared = Collection {
        id: "ctm".to_owned(),
        title: "Control of the ctm collection".to_owned(),
        visibility: "public".to_owned(),
        profiles: BTreeMap::from([
            ("chunking".to_owned(), "structural-400-600/2".to_owned()),
            ("sparse".to_owned(), "bm25-en-fr/1".to_owned()),
        ]),
    };
    database.record_collection(&declared).unwrap();
    assert_eq!(database.collection("ctm").unwrap(), Some(declared));
    assert_eq!(database.collection("other").unwrap(), None);
}

#[test]
fn a_source_is_recorded_in_its_collection_then_follows_its_declaration() {
    let scratch = Scratch::new();
    let database = scratch.empty();
    database.record_collection(&collection("ctm")).unwrap();
    database.record_source(&source("ctm", "docs")).unwrap();
    let recorded = Source {
        collection_id: "ctm".to_owned(),
        id: "docs".to_owned(),
        kind: "import".to_owned(),
        transport: None,
        reference: "corpus_root:docs.jsonl".to_owned(),
        profiles: BTreeMap::new(),
    };
    assert_eq!(database.source("ctm", "docs").unwrap(), Some(recorded));
    let declared = Source {
        collection_id: "ctm".to_owned(),
        id: "docs".to_owned(),
        kind: "web".to_owned(),
        transport: Some("http".to_owned()),
        reference: "https://example.org/".to_owned(),
        profiles: BTreeMap::from([("extraction".to_owned(), "technical-html/1".to_owned())]),
    };
    database.record_source(&declared).unwrap();
    assert_eq!(database.source("ctm", "docs").unwrap(), Some(declared));
    assert_eq!(database.source("ctm", "other").unwrap(), None);
}

#[test]
fn two_collections_may_each_declare_a_source_of_one_id() {
    let scratch = Scratch::new();
    let database = scratch.empty();
    for id in ["ctm", "synthetic"] {
        database.record_collection(&collection(id)).unwrap();
    }
    let mut synthetic = source("synthetic", "docs");
    synthetic.reference = "fixtures:synthetic.jsonl".to_owned();
    database.record_source(&source("ctm", "docs")).unwrap();
    database.record_source(&synthetic).unwrap();
    assert_eq!(
        database.source("ctm", "docs").unwrap(),
        Some(source("ctm", "docs"))
    );
    assert_eq!(
        database.source("synthetic", "docs").unwrap(),
        Some(synthetic)
    );
    assert_eq!(database.source("other", "docs").unwrap(), None);
}

#[test]
fn a_source_of_an_unrecorded_collection_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.empty();
    let error = database.record_source(&source("ctm", "docs")).unwrap_err();
    assert!(
        refused(&error, ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
        "{error:?}"
    );
    assert_eq!(database.source("ctm", "docs").unwrap(), None);
}

#[test]
fn a_document_is_recorded_once_and_recording_it_again_changes_nothing() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let recorded = Document {
        id: "doc-a".to_owned(),
        collection_id: "ctm".to_owned(),
        source_id: "docs".to_owned(),
        source_ref: "https://example.org/a".to_owned(),
    };
    assert_eq!(database.document("doc-a").unwrap(), Some(recorded.clone()));
    database.record_document(&recorded).unwrap();
    assert_eq!(database.document("doc-a").unwrap(), Some(recorded));
    assert_eq!(database.document("doc-b").unwrap(), None);
}

#[test]
fn a_document_recorded_again_elsewhere_is_refused_and_kept() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .record_collection(&collection("synthetic"))
        .unwrap();
    database
        .record_source(&source("synthetic", "docs"))
        .unwrap();
    database.record_source(&source("ctm", "kb")).unwrap();
    let kept = document("doc-a", "https://example.org/a");
    let mut elsewhere = [kept.clone(), kept.clone(), kept.clone()];
    elsewhere[0].collection_id = "synthetic".to_owned();
    elsewhere[1].source_id = "kb".to_owned();
    elsewhere[2].source_ref = "https://example.org/moved".to_owned();
    for moved in elsewhere {
        let error = database.record_document(&moved).unwrap_err();
        assert!(
            matches!(&error, Error::DocumentConflict(id) if id == "doc-a"),
            "{moved:?}: {error:?}"
        );
        assert_eq!(database.document("doc-a").unwrap().as_ref(), Some(&kept));
    }
}

#[test]
fn a_second_document_from_one_source_reference_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let error = database
        .record_document(&document("doc-b", "https://example.org/a"))
        .unwrap_err();
    assert!(refused(&error, ffi::SQLITE_CONSTRAINT_UNIQUE), "{error:?}");
    assert_eq!(database.document("doc-b").unwrap(), None);
}

#[test]
fn a_document_of_an_undeclared_source_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut undeclared = document("doc-b", "https://example.org/b");
    undeclared.source_id = "kb".to_owned();
    let error = database.record_document(&undeclared).unwrap_err();
    assert!(
        refused(&error, ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
        "{error:?}"
    );
    assert_eq!(database.document("doc-b").unwrap(), None);
}
