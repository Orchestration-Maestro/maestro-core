//! The guards of migration `0008_document_guards`: whoever writes, a
//! document keeps the id, collection, source and source reference it was
//! first recorded with, so neither it nor its revisions move to another
//! collection. Each trigger is shown refusing raw SQL the kernel's functions
//! never send.

use super::support::{Scratch, collection, revision, source};
use crate::{
    document::{Document, RevisionStatus},
    scope::ScopeSet,
    store::{self, Database},
};

/// Runs `statement` on the writer and commits what it did, or gives the
/// message of the refusal.
fn run(database: &Database, statement: &str) -> Result<(), String> {
    database
        .write(|transaction| Ok::<_, store::Error>(transaction.execute_batch(statement)))
        .unwrap()
        .map_err(|error| error.to_string())
}

/// The database of `scratch` with the document `doc-a` of the source `docs`
/// of `ctm` and its revision `rev-a`, a second source of `ctm`, `more`, and
/// the collection `other`, whose source is named `docs` too.
fn recorded(scratch: &Scratch) -> Database {
    let database = scratch.open();
    database
        .record_revision(&revision(&database, "rev-a", RevisionStatus::Valid))
        .unwrap();
    database.record_source(&source("ctm", "more")).unwrap();
    database.record_collection(&collection("other")).unwrap();
    database.record_source(&source("other", "docs")).unwrap();
    database
}

/// The document `doc-a`, as the database holds it.
fn doc_a(database: &Database) -> Option<Document> {
    database
        .document(&ScopeSet::default_workspace(), "doc-a")
        .unwrap()
}

#[test]
fn a_document_keeps_its_id_collection_source_and_source_reference() {
    let scratch = Scratch::new();
    let database = recorded(&scratch);
    let kept = doc_a(&database);
    for change in [
        "collection_id = 'other'",
        "source_id = 'more'",
        "source_ref = 'https://example.org/moved'",
        "id = 'doc-z'",
    ] {
        assert_eq!(
            run(
                &database,
                &format!("UPDATE documents SET {change} WHERE id = 'doc-a'")
            ),
            Err("a document keeps its id, collection, source and source reference".to_owned()),
            "{change}"
        );
        assert_eq!(doc_a(&database), kept, "{change}");
    }
    // Setting a column to its own value changes nothing.
    run(
        &database,
        "UPDATE documents SET collection_id = collection_id",
    )
    .unwrap();
}

#[test]
fn a_document_is_never_replaced() {
    let scratch = Scratch::new();
    let database = recorded(&scratch);
    let kept = doc_a(&database);
    let into = "INSERT INTO documents (id, collection_id, source_id, source_ref)";
    let replace = "INSERT OR REPLACE INTO documents (id, collection_id, source_id, source_ref)";
    for statement in [
        // Its id in another collection, replaced, then upserted.
        format!("{replace} VALUES ('doc-a', 'other', 'docs', 'https://example.org/a')"),
        format!(
            "{into} VALUES ('doc-a', 'other', 'docs', 'https://example.org/a')
             ON CONFLICT (id) DO UPDATE SET collection_id = excluded.collection_id"
        ),
        // Its source reference in its collection, under another id.
        format!("{replace} VALUES ('doc-b', 'ctm', 'docs', 'https://example.org/a')"),
    ] {
        assert_eq!(
            run(&database, &statement),
            Err(
                "a document is never replaced: no insert takes its id or its source reference"
                    .to_owned()
            ),
            "{statement}"
        );
        assert_eq!(doc_a(&database), kept, "{statement}");
    }
}
