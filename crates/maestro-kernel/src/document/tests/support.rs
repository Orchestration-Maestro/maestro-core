//! What the record tests share: a scratch database, and the collection,
//! source, document and revisions they record in it.

use crate::{
    artifact::Digest,
    document::{Collection, Document, Revision, RevisionStatus, Source},
    store::Database,
};
use serde_json::{Map, Value};
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
            "maestro-kernel-document-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The kernel database of this directory, with nothing recorded in it.
    pub(super) fn empty(&self) -> Database {
        Database::open_in(&self.0).unwrap()
    }

    /// The kernel database of this directory, with the collection `ctm`, its
    /// source `docs` and that source's document `doc-a` recorded.
    pub(super) fn open(&self) -> Database {
        let database = self.empty();
        database.record_collection(&collection("ctm")).unwrap();
        database.record_source(&source("ctm", "docs")).unwrap();
        database
            .record_document(&document("doc-a", "https://example.org/a"))
            .unwrap();
        database
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// The private collection `id`, chunked by `structural-500-700/1`.
pub(super) fn collection(id: &str) -> Collection {
    Collection {
        id: id.to_owned(),
        title: format!("The {id} collection"),
        visibility: "private".to_owned(),
        profiles: BTreeMap::from([("chunking".to_owned(), "structural-500-700/1".to_owned())]),
    }
}

/// The import source `id` of `collection`, whose manifest is
/// `corpus_root:<id>.jsonl`.
pub(super) fn source(collection: &str, id: &str) -> Source {
    Source {
        collection_id: collection.to_owned(),
        id: id.to_owned(),
        kind: "import".to_owned(),
        transport: None,
        reference: format!("corpus_root:{id}.jsonl"),
        profiles: BTreeMap::new(),
    }
}

/// The document `id` of the source `docs` of `ctm`, from `source_ref`.
pub(super) fn document(id: &str, source_ref: &str) -> Document {
    Document {
        id: id.to_owned(),
        collection_id: "ctm".to_owned(),
        source_id: "docs".to_owned(),
        source_ref: source_ref.to_owned(),
    }
}

/// The revision `id` of `doc-a` with `status`, titled after its id, whose
/// original and canonical artifacts `database` stores, without a pin.
pub(super) fn revision(database: &Database, id: &str, status: RevisionStatus) -> Revision {
    let original = database
        .put(format!("# {id}\n").as_bytes(), "text/markdown")
        .unwrap();
    let canonical = database
        .put(
            format!("{{\"revision_id\":\"{id}\"}}").as_bytes(),
            "application/json",
        )
        .unwrap();
    Revision {
        id: id.to_owned(),
        document_id: "doc-a".to_owned(),
        original_digest: original,
        canonical_digest: canonical,
        status,
        captured_at: None,
        metadata: Map::from_iter([("title".to_owned(), Value::from(format!("Page {id}")))]),
    }
}

/// The pins the database records for the artifact `digest`, which it
/// records.
pub(super) fn pins(database: &Database, digest: &Digest) -> u64 {
    database.artifact(digest).unwrap().unwrap().pins
}

/// The ids of `revisions`, in their order.
pub(super) fn ids(revisions: &[Revision]) -> Vec<&str> {
    revisions
        .iter()
        .map(|revision| revision.id.as_str())
        .collect()
}
