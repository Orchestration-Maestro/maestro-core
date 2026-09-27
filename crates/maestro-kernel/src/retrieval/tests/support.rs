//! Shared scratch records for controlled search-reader tests.

use crate::{
    chunk_set::{Chunk, NewChunkSet},
    document::{Collection, Document, Revision, RevisionStatus, Source},
    evidence::Span,
    generation::{Generation, NewGeneration},
    retrieval::{SearchInput, SearchMember},
    scope::{Right, ScopeSet},
    store::{self, Database},
};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A scratch kernel that owns one prepared chunk in a published-scope set.
pub(super) struct SearchDb {
    pub(super) database: Database,
    pub(super) scopes: ScopeSet,
    pub(super) generation: Generation,
    pub(super) chunk_id: String,
    pub(super) prepared_input: String,
    search_inputs: Vec<SearchInput>,
    _scratch: Scratch,
}

impl SearchDb {
    /// Records one collection, source, eligible revision, chunk and generation.
    pub(super) fn new(prepared_input: &str) -> Self {
        Self::with_inputs(&[prepared_input])
    }

    /// Records one or more prepared chunks in the pinned set.
    pub(super) fn with_inputs(prepared_inputs: &[&str]) -> Self {
        assert!(!prepared_inputs.is_empty());
        let scratch = Scratch::new();
        let database = Database::open_in(&scratch.0).unwrap();
        record_revision(&database);
        record_chunk_set(&database, prepared_inputs);
        let generation = database
            .create_generation(&NewGeneration {
                collection_id: "ctm".to_owned(),
                chunk_set_id: "set-a".to_owned(),
                embedding_profile: "embed:test".to_owned(),
                sparse_profile: "bm25-en-fr/1".to_owned(),
            })
            .unwrap();
        let collection_scope = "workspace/default/collection/ctm".parse().unwrap();
        database
            .grant("reader", &collection_scope, Right::Read, "test")
            .unwrap();
        let scopes = database.visible("reader").unwrap();
        Self {
            database,
            scopes,
            generation,
            chunk_id: "chunk-a".to_owned(),
            prepared_input: prepared_inputs[0].to_owned(),
            search_inputs: prepared_inputs
                .iter()
                .enumerate()
                .map(|(index, prepared_input)| SearchInput {
                    chunk_id: chunk_id(index),
                    prepared_input: (*prepared_input).to_owned(),
                })
                .collect(),
            _scratch: scratch,
        }
    }

    /// Adds a duplicate-content member revision without recording its own chunks.
    pub(super) fn add_duplicate_document(&self) {
        self.database
            .record_document(&Document {
                id: "doc-b".to_owned(),
                collection_id: "ctm".to_owned(),
                source_id: "docs".to_owned(),
                source_ref: "https://example.org/b".to_owned(),
            })
            .unwrap();
        let original = self.database.put(b"source text", "text/markdown").unwrap();
        let canonical = self
            .database
            .put(b"{\"duplicate\":true}", "application/json")
            .unwrap();
        self.database
            .record_revision(&Revision {
                id: "rev-b".to_owned(),
                document_id: "doc-b".to_owned(),
                original_digest: original,
                canonical_digest: canonical,
                status: RevisionStatus::Valid,
                captured_at: None,
                metadata: Map::from_iter([
                    ("set".to_owned(), Value::from("faq")),
                    ("version".to_owned(), Value::from("2.0")),
                ]),
            })
            .unwrap();
        self.database
            .write(|transaction| {
                transaction
                    .execute(
                        "INSERT INTO quality_dispositions
                         (revision_id, disposition, reasons_json, rule_ids, decided_by)
                         VALUES ('rev-b', 'accepted', '[]', '[]', 'test')",
                        [],
                    )
                    .map_err(store::Error::from)
            })
            .unwrap();
    }

    /// Records the exact prepared input and marks the generation's projection ready.
    pub(super) fn ready(&self) {
        self.ready_with_members(&[SearchMember {
            revision_id: "rev-a".to_owned(),
            representative_revision_id: "rev-a".to_owned(),
        }]);
    }

    /// Records members and exact prepared inputs, then marks the generation's
    /// search projection ready.
    pub(super) fn ready_with_members(&self, members: &[SearchMember]) {
        self.database
            .record_search_members(&self.scopes, &self.generation.chunk_set_id, members)
            .unwrap();
        self.database
            .record_search_inputs(
                &self.scopes,
                &self.generation.chunk_set_id,
                &self.search_inputs,
            )
            .unwrap();
        assert!(
            self.database
                .begin_generation_search(&self.scopes, self.generation.id, "identifiers/1")
                .unwrap()
        );
        self.database
            .complete_generation_search(&self.scopes, self.generation.id)
            .unwrap();
    }
}

/// Records the collection, source, document, accepted revision and disposition.
fn record_revision(database: &Database) {
    database
        .record_collection(&Collection {
            id: "ctm".to_owned(),
            title: "The ctm collection".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .record_source(&Source {
            collection_id: "ctm".to_owned(),
            id: "docs".to_owned(),
            kind: "import".to_owned(),
            transport: None,
            reference: "corpus_root:docs.jsonl".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .record_document(&Document {
            id: "doc-a".to_owned(),
            collection_id: "ctm".to_owned(),
            source_id: "docs".to_owned(),
            source_ref: "https://example.org/a".to_owned(),
        })
        .unwrap();
    let original = database.put(b"source text", "text/markdown").unwrap();
    let canonical = database.put(b"{}", "application/json").unwrap();
    database
        .record_revision(&Revision {
            id: "rev-a".to_owned(),
            document_id: "doc-a".to_owned(),
            original_digest: original,
            canonical_digest: canonical,
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::from_iter([
                ("set".to_owned(), Value::from("guide")),
                ("version".to_owned(), Value::from("1.2")),
            ]),
        })
        .unwrap();
    database
        .write(|transaction| {
            transaction
                .execute(
                    "INSERT INTO quality_dispositions
                     (revision_id, disposition, reasons_json, rule_ids, decided_by)
                     VALUES ('rev-a', 'accepted', '[]', '[]', 'test')",
                    [],
                )
                .map_err(store::Error::from)
        })
        .unwrap();
}

/// Records and completes a chunk set holding exactly the prepared input.
fn record_chunk_set(database: &Database, prepared_inputs: &[&str]) {
    database
        .begin_chunk_set(&NewChunkSet {
            id: "set-a",
            collection_id: "ctm",
            chunk_profile: "structural-500-700/1",
            counter_contract_id: "test/1",
        })
        .unwrap();
    let chunks = prepared_inputs
        .iter()
        .enumerate()
        .map(|(index, prepared_input)| Chunk {
            id: chunk_id(index),
            revision_id: "rev-a".to_owned(),
            section_id: None,
            digest: database
                .put(prepared_input.as_bytes(), "text/plain; charset=utf-8")
                .unwrap(),
            token_count: 3,
            span: Span { start: 0, end: 11 },
        })
        .collect::<Vec<_>>();
    database.record_chunks("set-a", "rev-a", &chunks).unwrap();
    let manifest = database.put(b"[]", "application/json").unwrap();
    database.complete_chunk_set("set-a", &manifest).unwrap();
}

/// The stable test chunk ID for a prepared-input position.
fn chunk_id(index: usize) -> String {
    match index {
        0 => "chunk-a".to_owned(),
        1 => "chunk-b".to_owned(),
        _ => format!("chunk-{index}"),
    }
}

/// A unique temporary directory, removed after its database is dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-search-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
