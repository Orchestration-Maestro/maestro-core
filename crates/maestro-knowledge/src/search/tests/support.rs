//! Scratch kernel records for the bounded candidate handoff.

use crate::search::SearchConfiguration;
use crate::search::{candidates, fusion::Fused};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    document::{Collection, Disposition, Document, Outcome, Revision, RevisionStatus, Source},
    evidence::Span,
    generation::{Generation, NewGeneration},
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::time::Instant;

/// One accepted chunk in a pinned scratch generation.
pub(super) struct CandidateDb {
    pub(super) database: Arc<Database>,
    pub(super) generation: Generation,
    pub(super) scopes: ScopeSet,
    pub(super) chunk_id: String,
    pub(super) revision_id: String,
    digest: Digest,
    scratch: Scratch,
}

impl CandidateDb {
    /// Records one accepted chunk owned by `source`, while only granting `docs`.
    pub(super) fn new(prepared_input: &[u8], source: &str) -> Self {
        let scratch = Scratch::new();
        let database = Arc::new(Database::open_in(&scratch.0).unwrap());
        let collection = format!("candidate-{}", NEXT.fetch_add(1, Ordering::Relaxed));
        let document = record_collection(&database, &collection, source);
        let revision_id = record_revision(&database, &collection, &document);
        let chunk_set = format!("set-{collection}");
        let (chunk_id, digest) = record_chunk_set(
            &database,
            &collection,
            &chunk_set,
            &revision_id,
            prepared_input,
        );
        let generation = database
            .create_generation(&NewGeneration {
                collection_id: collection,
                chunk_set_id: chunk_set,
                embedding_profile: "dense/test".to_owned(),
                sparse_profile: "bm25/test".to_owned(),
            })
            .unwrap();
        let scopes = database.visible("reader").unwrap();
        Self {
            database,
            generation,
            scopes,
            chunk_id,
            revision_id,
            digest,
            scratch,
        }
    }

    /// Replaces the stored bytes without changing their recorded digest.
    pub(super) fn corrupt_artifact(&self) {
        fs::write(self.artifact_path(), b"corrupt").unwrap();
    }

    /// Builds one candidate-load request for an arbitrary route claim.
    pub(super) fn request(
        &self,
        chunk_id: &str,
        expected_revisions: Vec<String>,
    ) -> candidates::Request {
        candidates::Request {
            generation: self.generation.clone(),
            scopes: self.scopes.clone(),
            version: None,
            fused: vec![Fused {
                chunk_id: chunk_id.to_owned(),
                score: 1.0,
                ranks: BTreeMap::new(),
            }],
            configuration: SearchConfiguration::default(),
            query: "query".to_owned(),
            source_classes: None,
            expected_revisions: HashMap::from([(chunk_id.to_owned(), expected_revisions)]),
            deadline: Instant::now() + Duration::from_secs(2),
            context_deadline: Instant::now() + Duration::from_secs(2),
        }
    }

    fn artifact_path(&self) -> PathBuf {
        let digest = self.digest.as_str();
        self.scratch
            .0
            .join("artifacts")
            .join("sha256")
            .join(&digest[..2])
            .join(&digest[2..4])
            .join(digest)
    }
}

fn record_collection(database: &Database, collection: &str, source: &str) -> String {
    let source_scope: Scope = format!("workspace/default/collection/{collection}/source/docs")
        .parse()
        .unwrap();
    database
        .grant("reader", &source_scope, Right::Read, "test")
        .unwrap();
    database
        .record_collection(&Collection {
            id: collection.to_owned(),
            title: "Candidate handoff".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    for id in ["docs", "private"] {
        database
            .record_source(&Source {
                collection_id: collection.to_owned(),
                id: id.to_owned(),
                kind: "import".to_owned(),
                transport: None,
                reference: format!("corpus_root:{id}.jsonl"),
                profiles: BTreeMap::new(),
            })
            .unwrap();
    }
    let document = format!("doc-{collection}");
    database
        .record_document(&Document {
            id: document.clone(),
            collection_id: collection.to_owned(),
            source_id: source.to_owned(),
            source_ref: "https://example.org/candidate".to_owned(),
        })
        .unwrap();
    document
}

fn record_revision(database: &Database, collection: &str, document: &str) -> String {
    let original_digest = database.put(b"source", "text/markdown").unwrap();
    let canonical_digest = database.put(b"{}", "application/json").unwrap();
    let revision_id = format!("revision-{collection}");
    database
        .record_revision(&Revision {
            id: revision_id.clone(),
            document_id: document.to_owned(),
            original_digest,
            canonical_digest,
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::<String, Value>::new(),
        })
        .unwrap();
    database
        .record_disposition(&Disposition {
            revision_id: revision_id.clone(),
            outcome: Outcome::Accepted,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: "test".to_owned(),
        })
        .unwrap();
    revision_id
}

fn record_chunk_set(
    database: &Database,
    collection: &str,
    chunk_set: &str,
    revision_id: &str,
    prepared_input: &[u8],
) -> (String, Digest) {
    database
        .begin_chunk_set(&NewChunkSet {
            id: chunk_set,
            collection_id: collection,
            chunk_profile: "mapped-structural-chunks/2",
            counter_contract_id: "router/1:sha256:test",
        })
        .unwrap();
    let chunk_id = format!("chunk-{collection}");
    let digest = database
        .put(prepared_input, "text/plain; charset=utf-8")
        .unwrap();
    database
        .record_chunks(
            chunk_set,
            revision_id,
            &[Chunk {
                id: chunk_id.clone(),
                revision_id: revision_id.to_owned(),
                section_id: None,
                digest: digest.clone(),
                token_count: 1,
                span: Span { start: 0, end: 1 },
            }],
        )
        .unwrap();
    let manifest = database.put(b"[]", "application/json").unwrap();
    database.complete_chunk_set(chunk_set, &manifest).unwrap();
    (chunk_id, digest)
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        Self(scratch_directory().unwrap())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
