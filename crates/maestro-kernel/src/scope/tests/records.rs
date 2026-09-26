//! The readers of the pipeline's records take the caller's `ScopeSet` and
//! filter inside their query: a collection and its generations have the
//! collection's scope, and a source, its documents and their revisions the
//! source's. What a set does not cover reads as absent, and a revocation
//! applies to the next read, even of a retired generation.

use super::support::{Scratch, scope};
use crate::{
    document::{Collection, Document, Revision, RevisionStatus, Source},
    generation::NewGeneration,
    scope::{Right, ScopeSet},
    store::{self, Database},
};
use serde_json::Map;
use std::collections::BTreeMap;

/// The collections the tests record: the name of one is a prefix of the
/// other's.
const COLLECTIONS: [&str; 2] = ["ctm", "ct"];
/// The sources the tests record, each as its collection and its id.
const SOURCES: [(&str, &str); 3] = [("ctm", "docs-core"), ("ctm", "guides"), ("ct", "docs-core")];
/// The document of each source, in the order of [`SOURCES`].
const DOCUMENTS: [&str; 3] = ["doc-a", "doc-b", "doc-c"];
/// The revision of each document, in the order of [`DOCUMENTS`].
const REVISIONS: [&str; 3] = ["rev-a", "rev-b", "rev-c"];

/// What a set that covers `ctm` reads: every record of `ctm`, none of `ct`.
const CTM: [&str; 14] = [
    "collection ctm",
    "source ctm/docs-core",
    "source ctm/guides",
    "document doc-a",
    "document doc-b",
    "revision rev-a",
    "revision rev-b",
    "eligible rev-a",
    "eligible rev-b",
    "listed rev-a",
    "listed rev-b",
    "generation ctm Retired",
    "generation ctm Published",
    "published ctm",
];

/// The private collection `id`.
fn collection(id: &str) -> Collection {
    Collection {
        id: id.to_owned(),
        title: format!("The {id} collection"),
        visibility: "private".to_owned(),
        profiles: BTreeMap::new(),
    }
}

/// The import source `id` of `collection`.
fn source(collection: &str, id: &str) -> Source {
    Source {
        collection_id: collection.to_owned(),
        id: id.to_owned(),
        kind: "import".to_owned(),
        transport: None,
        reference: format!("corpus_root:{id}.jsonl"),
        profiles: BTreeMap::new(),
    }
}

/// Records each collection with a chunk set, each source with a document and
/// its revision, then two generations of each collection, the second
/// published after the first, which it retires; returns the generations'
/// ids in the order they were created.
fn record(database: &Database) -> Vec<i64> {
    for id in COLLECTIONS {
        database.record_collection(&collection(id)).unwrap();
        database
            .write(|transaction| {
                transaction.execute(
                    "INSERT INTO chunk_sets (id, collection_id, chunk_profile,
                       counter_contract_id, state)
                     VALUES (?1 || '-set', ?1, 'structural-500-700/1', 'native', 'complete')",
                    [id],
                )?;
                Ok::<_, store::Error>(())
            })
            .unwrap();
    }
    for (((collection, source), document), revision) in
        SOURCES.into_iter().zip(DOCUMENTS).zip(REVISIONS)
    {
        record_revision(database, collection, source, document, revision);
    }
    let mut generations = Vec::new();
    for collection in COLLECTIONS {
        for _ in 0..2 {
            let new = NewGeneration {
                collection_id: collection.to_owned(),
                chunk_set_id: format!("{collection}-set"),
                embedding_profile: "embed:test".to_owned(),
                sparse_profile: "bm25-en-fr/1".to_owned(),
            };
            let id = database.create_generation(&new).unwrap().id;
            database.verify_generation(id, 3).unwrap();
            database.publish_generation(id).unwrap();
            generations.push(id);
        }
    }
    generations
}

/// Records the source `source_id` of `collection`, its document `document`
/// and that document's revision `revision`, with its two artifacts.
fn record_revision(
    database: &Database,
    collection: &str,
    source_id: &str,
    document: &str,
    revision: &str,
) {
    database
        .record_source(&source(collection, source_id))
        .unwrap();
    database
        .record_document(&Document {
            id: document.to_owned(),
            collection_id: collection.to_owned(),
            source_id: source_id.to_owned(),
            source_ref: format!("https://example.org/{collection}/{document}"),
        })
        .unwrap();
    let original = database
        .put(format!("# {revision}\n").as_bytes(), "text/markdown")
        .unwrap();
    let canonical = database
        .put(
            format!("{{\"revision_id\":\"{revision}\"}}").as_bytes(),
            "application/json",
        )
        .unwrap();
    database
        .record_revision(&Revision {
            id: revision.to_owned(),
            document_id: document.to_owned(),
            original_digest: original,
            canonical_digest: canonical,
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::new(),
        })
        .unwrap();
}

/// The set of `principal`, once granted the scope `path`.
fn granted(database: &Database, principal: &str, path: &str) -> ScopeSet {
    database
        .grant(principal, &scope(path), Right::Read, "test")
        .unwrap();
    database.visible(principal).unwrap()
}

/// What the readers of records read with `scopes`, one label a record: each
/// reader is asked for every record [`record`] made, `generations` the ids
/// of its generations.
fn seen(database: &Database, scopes: &ScopeSet, generations: &[i64]) -> Vec<String> {
    let mut seen: Vec<String> = COLLECTIONS
        .into_iter()
        .filter_map(|id| database.collection(scopes, id).unwrap())
        .map(|collection| format!("collection {}", collection.id))
        .collect();
    seen.extend(
        SOURCES
            .into_iter()
            .filter_map(|(collection, id)| database.source(scopes, collection, id).unwrap())
            .map(|source| format!("source {}/{}", source.collection_id, source.id)),
    );
    seen.extend(
        DOCUMENTS
            .into_iter()
            .filter_map(|id| database.document(scopes, id).unwrap())
            .map(|document| format!("document {}", document.id)),
    );
    seen.extend(
        REVISIONS
            .into_iter()
            .filter_map(|id| database.revision(scopes, id).unwrap())
            .map(|revision| format!("revision {}", revision.id)),
    );
    seen.extend(
        COLLECTIONS
            .into_iter()
            .flat_map(|id| database.eligible_revisions(scopes, id).unwrap())
            .map(|revision| format!("eligible {}", revision.id)),
    );
    seen.extend(
        COLLECTIONS
            .into_iter()
            .flat_map(|id| database.revisions(scopes, id).unwrap())
            .map(|revision| format!("listed {}", revision.id)),
    );
    seen.extend(
        generations
            .iter()
            .filter_map(|id| database.generation(scopes, *id).unwrap())
            .map(|generation| {
                format!(
                    "generation {} {:?}",
                    generation.collection_id, generation.state
                )
            }),
    );
    seen.extend(
        COLLECTIONS
            .into_iter()
            .filter_map(|id| database.published_generation(scopes, id).unwrap())
            .map(|generation| format!("published {}", generation.collection_id)),
    );
    seen
}

#[test]
fn each_reader_of_records_reads_only_inside_the_set() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let generations = record(&database);
    let ctm = granted(&database, "ctm-reader", "workspace/default/collection/ctm");
    assert_eq!(seen(&database, &ctm, &generations), CTM);
    let ct = granted(&database, "ct-reader", "workspace/default/collection/ct");
    assert_eq!(
        seen(&database, &ct, &generations),
        [
            "collection ct",
            "source ct/docs-core",
            "document doc-c",
            "revision rev-c",
            "eligible rev-c",
            "listed rev-c",
            "generation ct Retired",
            "generation ct Published",
            "published ct",
        ]
    );
    let docs = granted(
        &database,
        "docs-reader",
        "workspace/default/collection/ctm/source/docs-core",
    );
    assert_eq!(
        seen(&database, &docs, &generations),
        [
            "source ctm/docs-core",
            "document doc-a",
            "revision rev-a",
            "eligible rev-a",
            "listed rev-a",
        ]
    );
}

#[test]
fn an_empty_set_reads_no_record() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let generations = record(&database);
    let nothing = database.visible("nobody").unwrap();
    assert_eq!(seen(&database, &nothing, &generations), [""; 0]);
}

#[test]
fn a_revocation_hides_every_record_on_the_next_read_a_retired_generation_included() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let generations = record(&database);
    let before = granted(&database, "local", "workspace/default/collection/ctm");
    assert_eq!(seen(&database, &before, &generations), CTM);
    database
        .revoke(
            "local",
            &scope("workspace/default/collection/ctm"),
            Right::Read,
            "test",
        )
        .unwrap();
    let after = database.visible("local").unwrap();
    assert_eq!(seen(&database, &after, &generations), [""; 0]);
}

#[test]
fn a_grant_on_a_collection_reads_no_record_of_another_collection() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database);
    let ctm = granted(&database, "ctm-reader", "workspace/default/collection/ctm");
    for id in ["ctm/source/x", "ctm/"] {
        let recorded = database.record_collection(&collection(id));
        assert_eq!(database.collection(&ctm, id).unwrap(), None, "{id}");
        assert!(recorded.is_err(), "{id} was recorded");
    }
}

#[test]
fn a_grant_on_a_source_reads_no_record_of_another_source() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database);
    let docs = granted(
        &database,
        "docs-reader",
        "workspace/default/collection/ctm/source/docs-core",
    );
    let recorded = database.record_source(&source("ctm", "docs-core/x"));
    assert_eq!(database.source(&docs, "ctm", "docs-core/x").unwrap(), None);
    assert!(recorded.is_err(), "docs-core/x was recorded");
}
