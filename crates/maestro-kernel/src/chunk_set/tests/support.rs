//! What the chunk set tests share: a scratch database holding two revisions
//! of a document of the collection `ctm` and one of a document of the
//! collection `other`, the chunk sets they begin, the prepared inputs their
//! chunks pin, and the scopes of one collection.

use crate::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    document::{Collection, Document, Revision, RevisionStatus, Source},
    evidence::Span,
    scope::{Right, ScopeSet},
    store::Database,
};
use serde_json::Map;
use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: after the database a test opened in
/// it, which it declares later.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-chunk-set-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The kernel database of this directory, with the revisions `rev-a` and
    /// `rev-b` of the document `doc-a` of the collection `ctm`, and `rev-z`
    /// of the document `doc-z` of the collection `other`, each in the source
    /// `docs` of its collection.
    pub(super) fn open(&self) -> Database {
        let database = Database::open_in(&self.0).unwrap();
        for (collection, document, revisions) in [
            ("ctm", "doc-a", ["rev-a", "rev-b"].as_slice()),
            ("other", "doc-z", &["rev-z"]),
        ] {
            record_document(&database, collection, document);
            for revision in revisions {
                record_revision(&database, document, revision);
            }
        }
        database
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Records the collection `collection`, its source `docs` and that source's
/// document `document`.
fn record_document(database: &Database, collection: &str, document: &str) {
    database
        .record_collection(&Collection {
            id: collection.to_owned(),
            title: format!("The {collection} collection"),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .record_source(&Source {
            collection_id: collection.to_owned(),
            id: "docs".to_owned(),
            kind: "import".to_owned(),
            transport: None,
            reference: "corpus_root:docs.jsonl".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .record_document(&Document {
            id: document.to_owned(),
            collection_id: collection.to_owned(),
            source_id: "docs".to_owned(),
            source_ref: format!("https://example.org/{document}"),
        })
        .unwrap();
}

/// Records the revision `id` of `document`, with artifacts of its own.
fn record_revision(database: &Database, document: &str, id: &str) {
    let original = database
        .put(format!("# {id}\n").as_bytes(), "text/markdown")
        .unwrap();
    let canonical = database
        .put(
            format!("{{\"revision\":\"{id}\"}}").as_bytes(),
            "application/json",
        )
        .unwrap();
    database
        .record_revision(&Revision {
            id: id.to_owned(),
            document_id: document.to_owned(),
            original_digest: original,
            canonical_digest: canonical,
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::new(),
        })
        .unwrap();
}

/// The chunk set `id` of `collection`, chunked by `mapped-structural-chunks/2`
/// and counted by `router/1:test`.
pub(super) fn new_set<'a>(id: &'a str, collection: &'a str) -> NewChunkSet<'a> {
    NewChunkSet {
        id,
        collection_id: collection,
        chunk_profile: "mapped-structural-chunks/2",
        counter_contract_id: "router/1:test",
    }
}

/// Stores `text` as a prepared input, without a pin, and returns its digest.
pub(super) fn prepared(database: &Database, text: &str) -> Digest {
    database
        .put(text.as_bytes(), "text/plain; charset=utf-8")
        .unwrap()
}

/// The chunk `id` of `revision`, over the bytes `start` to `end` of its
/// original, whose prepared input is `digest` and counts `tokens` tokens.
pub(super) fn chunk(
    id: &str,
    revision: &str,
    digest: &Digest,
    tokens: u64,
    span: [usize; 2],
) -> Chunk {
    Chunk {
        id: id.to_owned(),
        revision_id: revision.to_owned(),
        section_id: Some(format!("section-{id}")),
        digest: digest.clone(),
        token_count: tokens,
        span: Span {
            start: span[0],
            end: span[1],
        },
    }
}

/// The scopes of a principal granted the collection `collection` alone.
pub(super) fn reading(database: &Database, collection: &str) -> ScopeSet {
    let scope = format!("workspace/default/collection/{collection}")
        .parse()
        .unwrap();
    database
        .grant(collection, &scope, Right::Read, "test")
        .unwrap();
    database.visible(collection).unwrap()
}

/// The pins the database records for the artifact `digest`.
pub(super) fn pins(database: &Database, digest: &Digest) -> u64 {
    database.artifact(digest).unwrap().unwrap().pins
}
