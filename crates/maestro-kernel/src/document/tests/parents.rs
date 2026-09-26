//! Collections, their sources and their documents: collections and sources
//! follow their declaration, while a document keeps the collection, source and
//! source reference it was first recorded with.

use super::support::{Scratch, collection, document, source};
use crate::{
    document::{Collection, Document, Error, Source},
    scope::{ScopeSet, check_name},
    store,
};
use rusqlite::ffi;
use std::{collections::BTreeMap, error};

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
    assert_eq!(
        database
            .collection(&ScopeSet::default_workspace(), "ctm")
            .unwrap(),
        None
    );
    database.record_collection(&collection("ctm")).unwrap();
    let recorded = Collection {
        id: "ctm".to_owned(),
        title: "The ctm collection".to_owned(),
        visibility: "private".to_owned(),
        profiles: BTreeMap::from([("chunking".to_owned(), "structural-500-700/1".to_owned())]),
    };
    assert_eq!(
        database
            .collection(&ScopeSet::default_workspace(), "ctm")
            .unwrap(),
        Some(recorded)
    );
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
    assert_eq!(
        database
            .collection(&ScopeSet::default_workspace(), "ctm")
            .unwrap(),
        Some(declared)
    );
    assert_eq!(
        database
            .collection(&ScopeSet::default_workspace(), "other")
            .unwrap(),
        None
    );
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
    assert_eq!(
        database
            .source(&ScopeSet::default_workspace(), "ctm", "docs")
            .unwrap(),
        Some(recorded)
    );
    let declared = Source {
        collection_id: "ctm".to_owned(),
        id: "docs".to_owned(),
        kind: "web".to_owned(),
        transport: Some("http".to_owned()),
        reference: "https://example.org/".to_owned(),
        profiles: BTreeMap::from([("extraction".to_owned(), "technical-html/1".to_owned())]),
    };
    database.record_source(&declared).unwrap();
    assert_eq!(
        database
            .source(&ScopeSet::default_workspace(), "ctm", "docs")
            .unwrap(),
        Some(declared)
    );
    assert_eq!(
        database
            .source(&ScopeSet::default_workspace(), "ctm", "other")
            .unwrap(),
        None
    );
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
        database
            .source(&ScopeSet::default_workspace(), "ctm", "docs")
            .unwrap(),
        Some(source("ctm", "docs"))
    );
    assert_eq!(
        database
            .source(&ScopeSet::default_workspace(), "synthetic", "docs")
            .unwrap(),
        Some(synthetic)
    );
    assert_eq!(
        database
            .source(&ScopeSet::default_workspace(), "other", "docs")
            .unwrap(),
        None
    );
}

/// Checks that `error` refuses `id` as no scope name, naming it, with the
/// name's refusal as its source.
fn assert_invalid_id(error: &Error, id: &str) {
    let expected = check_name(id).unwrap_err();
    assert!(
        matches!(error, Error::InvalidId(invalid) if *invalid == expected),
        "{id}: {error:?}"
    );
    assert!(error.to_string().contains(&format!("`{id}`")), "{error}");
    let reason = error::Error::source(error).map(ToString::to_string);
    assert_eq!(reason, Some(expected.to_string()));
}

#[test]
fn an_id_that_is_no_scope_name_is_refused_before_anything_is_recorded() {
    let scratch = Scratch::new();
    let database = scratch.empty();
    database.record_collection(&collection("ctm")).unwrap();
    for id in ["ctm/source/x", "a/b", "ctm/"] {
        let as_collection = database.record_collection(&collection(id)).unwrap_err();
        assert_invalid_id(&as_collection, id);
        let as_source = database.record_source(&source("ctm", id)).unwrap_err();
        assert_invalid_id(&as_source, id);
        let as_its_collection = database.record_source(&source(id, "docs")).unwrap_err();
        assert_invalid_id(&as_its_collection, id);
    }
    let recorded: (i64, i64) = database
        .reader()
        .unwrap()
        .query_row(
            "SELECT (SELECT count(*) FROM collections), (SELECT count(*) FROM sources)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(recorded, (1, 0), "ctm alone is recorded");
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
    assert_eq!(
        database
            .source(&ScopeSet::default_workspace(), "ctm", "docs")
            .unwrap(),
        None
    );
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
    assert_eq!(
        database
            .document(&ScopeSet::default_workspace(), "doc-a")
            .unwrap(),
        Some(recorded.clone())
    );
    database.record_document(&recorded).unwrap();
    assert_eq!(
        database
            .document(&ScopeSet::default_workspace(), "doc-a")
            .unwrap(),
        Some(recorded)
    );
    assert_eq!(
        database
            .document(&ScopeSet::default_workspace(), "doc-b")
            .unwrap(),
        None
    );
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
        assert_eq!(
            database
                .document(&ScopeSet::default_workspace(), "doc-a")
                .unwrap()
                .as_ref(),
            Some(&kept)
        );
    }
}

#[test]
fn a_source_reference_names_one_document_in_each_collection() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let error = database
        .record_document(&document("doc-b", "https://example.org/a"))
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::SourceRefConflict { recorded, given } if recorded == "doc-a" && given == "doc-b"
        ),
        "{error:?}"
    );
    assert_eq!(
        database
            .document(&ScopeSet::default_workspace(), "doc-b")
            .unwrap(),
        None
    );
    database
        .record_collection(&collection("synthetic"))
        .unwrap();
    database
        .record_source(&source("synthetic", "docs"))
        .unwrap();
    let mut elsewhere = document("doc-b", "https://example.org/a");
    elsewhere.collection_id = "synthetic".to_owned();
    database.record_document(&elsewhere).unwrap();
    assert_eq!(
        database
            .document(&ScopeSet::default_workspace(), "doc-b")
            .unwrap(),
        Some(elsewhere)
    );
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
    assert_eq!(
        database
            .document(&ScopeSet::default_workspace(), "doc-b")
            .unwrap(),
        None
    );
}
