//! What the preparation's tests share: a scratch directory holding a corpus
//! and a kernel database, a collection imported from that corpus and given
//! its quality dispositions, and a router tokenizer qualified over the port
//! that answers the native goldens.

use super::{port::Goldens, support::embedder};
use crate::{
    collection::Declaration,
    import,
    prepare::{Report, RouterTokenizer},
};
use maestro_kernel::{
    artifact::Digest,
    binding::Bindings,
    chunk_set::{Chunk, ChunkSet},
    document::{Disposition, Outcome},
    scope::{Right, ScopeSet},
    store::Database,
};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::{
    env,
    fmt::Write as _,
    fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The collection every test prepares.
pub(crate) const COLLECTION: &str = "notes";

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: `corpus/` holds the documents and
/// their manifest, and `kernel/` the kernel's data.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-knowledge-preparation-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("corpus")).unwrap();
        Self(path)
    }

    /// The kernel's database, in `kernel/`.
    pub(crate) fn database(&self) -> Database {
        Database::open_in(&self.0.join("kernel")).unwrap()
    }

    /// How many rows the kernel's table `table` holds, read around the
    /// kernel's readers.
    pub(super) fn rows(&self, table: &str) -> u64 {
        let path = self.0.join("kernel").join("kernel.sqlite3");
        let connection =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let rows: i64 = connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        u64::try_from(rows).unwrap()
    }

    /// Writes each document of `documents`, a path relative to the corpus
    /// directory and its Markdown, and the manifest `manifest.jsonl` that
    /// declares them in that order, each titled `Notes` and from the
    /// `source_ref` `https://example.org/<path>`.
    pub(crate) fn corpus(&self, documents: &[(&str, &str)]) {
        let titled: Vec<_> = documents
            .iter()
            .map(|(path, markdown)| (*path, "Notes", *markdown))
            .collect();
        self.titled_corpus(&titled);
    }

    /// [`Scratch::corpus`], each document with its path, its title and its
    /// Markdown.
    pub(super) fn titled_corpus(&self, documents: &[(&str, &str, &str)]) {
        let mut manifest = String::new();
        for (path, title, markdown) in documents {
            let file = self.0.join("corpus").join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, markdown).unwrap();
            let line = json!({
                "schema": "maestro-corpus/1",
                "path": path,
                "sha256": Digest::of(markdown.as_bytes()).as_str(),
                "bytes": markdown.len(),
                "source_ref": format!("https://example.org/{path}"),
                "title": title,
                "source_kind": "guide",
            });
            writeln!(manifest, "{line}").unwrap();
        }
        fs::write(self.0.join("corpus").join("manifest.jsonl"), manifest).unwrap();
    }

    /// Imports the corpus as the collection [`COLLECTION`], whose one source
    /// `docs` reads `manifest.jsonl`, and returns what a principal granted
    /// the whole default workspace reads.
    pub(crate) fn import(&self, database: &Database) -> ScopeSet {
        let declaration: Declaration = json!({
            "schema": "maestro-collection/1",
            "id": COLLECTION,
            "title": "Notes",
            "visibility": "private",
            "profiles": {
                "extraction": "technical-html/1",
                "chunking": "mapped-structural-chunks/2",
                "embedding": "embed:winner",
                "sparse": "bm25-en-fr/1",
            },
            "quality": { "ledger": "quality/ledger.jsonl" },
            "sources": [{
                "id": "docs",
                "kind": "import",
                "sync": "manual",
                "manifest": { "binding": "corpus_root", "path": "manifest.jsonl" },
            }],
            "evals": { "suite": "evals" },
        })
        .to_string()
        .parse()
        .unwrap();
        let bindings: Bindings = format!("corpus_root = '{}'\n", self.0.join("corpus").display())
            .parse()
            .unwrap();
        let scopes = everything(database);
        let report = import::import(database, &scopes, &declaration, &bindings).unwrap();
        assert_eq!(report.refused, 0, "{:?}", report.refusals);
        scopes
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// What a principal granted the whole default workspace reads.
pub(super) fn everything(database: &Database) -> ScopeSet {
    let workspace = "workspace/default".parse().unwrap();
    database
        .grant("tester", &workspace, Right::Read, "test")
        .unwrap();
    database.visible("tester").unwrap()
}

/// The latest revision, in record order, of the document whose `path` the
/// corpus holds, as the import recorded it.
pub(crate) fn revision_of(database: &Database, scopes: &ScopeSet, path: &str) -> String {
    revisions_of(database, scopes, path).pop().unwrap()
}

/// Every revision of the document whose `path` the corpus holds, failed
/// ones included, in record order.
pub(super) fn revisions_of(database: &Database, scopes: &ScopeSet, path: &str) -> Vec<String> {
    let source_ref = format!("https://example.org/{path}");
    database
        .revisions(scopes, COLLECTION)
        .unwrap()
        .into_iter()
        .filter(|revision| {
            let document = database.document(scopes, &revision.document_id).unwrap();
            document.is_some_and(|document| document.source_ref == source_ref)
        })
        .map(|revision| revision.id)
        .collect()
}

/// The ID of the document whose `path` the corpus holds.
pub(super) fn document_of(database: &Database, scopes: &ScopeSet, path: &str) -> String {
    let revision = revision_of(database, scopes, path);
    database
        .revision(scopes, &revision)
        .unwrap()
        .unwrap()
        .document_id
}

/// Gives every revision of [`COLLECTION`] that has none the disposition
/// `outcome`, as the quality gate would.
pub(crate) fn decide_all(database: &Database, scopes: &ScopeSet, outcome: Outcome) {
    for revision in database.eligible_revisions(scopes, COLLECTION).unwrap() {
        decide(database, &revision.id, outcome);
    }
}

/// Gives the revision `revision` the disposition `outcome`, unless it has
/// one.
pub(super) fn decide(database: &Database, revision: &str, outcome: Outcome) {
    database
        .record_disposition(&Disposition {
            revision_id: revision.to_owned(),
            outcome,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: "test".to_owned(),
        })
        .unwrap();
}

/// A router tokenizer qualified over a port that answers the native goldens,
/// and any other text with one token per word, and that port.
pub(super) fn tokenizer() -> (Goldens, RouterTokenizer) {
    let port = Goldens::new();
    let tokenizer = RouterTokenizer::qualify(port.clone(), embedder()).unwrap();
    (port, tokenizer)
}

/// A qualified deterministic test counter for cross-module evidence tests.
pub(crate) fn router_tokenizer() -> RouterTokenizer {
    tokenizer().1
}

/// The chunk set `report` names, as the kernel records it.
pub(super) fn chunk_set_of(database: &Database, scopes: &ScopeSet, report: &Report) -> ChunkSet {
    database
        .chunk_set(scopes, &report.chunk_set)
        .unwrap()
        .unwrap()
}

/// The chunks of the chunk set `report` names.
pub(super) fn chunks_of(database: &Database, scopes: &ScopeSet, report: &Report) -> Vec<Chunk> {
    database.chunks(scopes, &report.chunk_set).unwrap()
}

/// The manifest of the chunk set `set`, as JSON.
pub(super) fn manifest_of(database: &Database, set: &ChunkSet) -> Value {
    let digest = set.manifest_digest.as_ref().unwrap();
    serde_json::from_slice(&database.get(digest).unwrap()).unwrap()
}

/// Changes every chunk in `chunk_set` to report the same section identity.
pub(super) fn set_chunk_sections(scratch: &Scratch, chunk_set: &str, section_id: &str) {
    let path = scratch.0.join("kernel").join("kernel.sqlite3");
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    connection
        .execute_batch("DROP TRIGGER chunks_never_change")
        .unwrap();
    connection
        .execute(
            "UPDATE chunks SET section_id = ?1 WHERE chunk_set_id = ?2",
            [section_id, chunk_set],
        )
        .unwrap();
}

/// Replaces an artifact's bytes without changing the digest in the record.
pub(crate) fn corrupt_artifact(scratch: &Scratch, digest: &Digest) {
    fs::write(artifact_path(scratch, digest), b"corrupt").unwrap();
}

/// Removes the file of a recorded artifact while retaining its metadata row.
pub(super) fn remove_artifact(scratch: &Scratch, digest: &Digest) {
    fs::remove_file(artifact_path(scratch, digest)).unwrap();
}

/// Moves one revision to the kernel's permitted failed status for reader tests.
pub(crate) fn fail_revision(scratch: &Scratch, revision_id: &str) {
    let path = scratch.0.join("kernel").join("kernel.sqlite3");
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    connection
        .execute(
            "UPDATE revisions SET status = 'failed' WHERE id = ?1",
            [revision_id],
        )
        .unwrap();
}

/// Changes a revision's one-time disposition for eligibility reader tests.
pub(crate) fn quarantine_revision(scratch: &Scratch, revision_id: &str) {
    let path = scratch.0.join("kernel").join("kernel.sqlite3");
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    connection
        .execute(
            "UPDATE quality_dispositions SET disposition = 'quarantined' WHERE revision_id = ?1",
            [revision_id],
        )
        .unwrap();
}

/// Gives a revision an accepted-with-warnings disposition for evidence tests.
pub(crate) fn accept_with_warnings(scratch: &Scratch, revision_id: &str) {
    let path = scratch.0.join("kernel").join("kernel.sqlite3");
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    connection
        .execute(
            "UPDATE quality_dispositions
             SET disposition = 'accepted_with_warnings',
                 reasons_json = '[\"approved warning\"]', rule_ids = '[\"test.warning\"]'
             WHERE revision_id = ?1",
            [revision_id],
        )
        .unwrap();
}

/// Adds one literal metadata field to a revision for reader tests.
pub(crate) fn set_revision_metadata(scratch: &Scratch, revision_id: &str, key: &str, value: &str) {
    let path = scratch.0.join("kernel").join("kernel.sqlite3");
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    connection
        .execute_batch("DROP TRIGGER IF EXISTS revisions_are_immutable")
        .unwrap();
    let stored: String = connection
        .query_row(
            "SELECT metadata_json FROM revisions WHERE id = ?1",
            [revision_id],
            |row| row.get(0),
        )
        .unwrap();
    let mut metadata: serde_json::Map<String, Value> = serde_json::from_str(&stored).unwrap();
    metadata.insert(key.to_owned(), Value::String(value.to_owned()));
    connection
        .execute(
            "UPDATE revisions SET metadata_json = ?1 WHERE id = ?2",
            [Value::Object(metadata).to_string(), revision_id.to_owned()],
        )
        .unwrap();
}

/// Repoints a revision to a newly stored canonical artifact for integrity tests.
pub(crate) fn replace_revision_canonical(
    scratch: &Scratch,
    database: &Database,
    revision_id: &str,
    bytes: &[u8],
) {
    let digest = database.put(bytes, "application/json").unwrap();
    let path = scratch.0.join("kernel").join("kernel.sqlite3");
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    connection
        .execute_batch("DROP TRIGGER IF EXISTS revisions_are_immutable")
        .unwrap();
    connection
        .execute(
            "UPDATE revisions SET canonical_digest = ?1 WHERE id = ?2",
            [digest.as_str(), revision_id],
        )
        .unwrap();
}

/// Locates the artifact `digest` in the scratch kernel's content-addressed tree.
fn artifact_path(scratch: &Scratch, digest: &Digest) -> PathBuf {
    let hex = digest.as_str();
    let (first, rest) = hex.split_at(2);
    let (second, _) = rest.split_at(2);
    scratch
        .0
        .join("kernel")
        .join("artifacts")
        .join("sha256")
        .join(first)
        .join(second)
        .join(hex)
}

/// Removes the chunk-set table so the next kernel lookup has a store error.
pub(super) fn drop_chunk_sets_table(scratch: &Scratch) {
    let path = scratch.0.join("kernel").join("kernel.sqlite3");
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = OFF; DROP TABLE chunk_sets")
        .unwrap();
}

/// Clears a complete set's manifest reference to exercise the reader refusal.
pub(crate) fn clear_chunk_set_manifest(scratch: &Scratch, chunk_set: &str) {
    let path = scratch.0.join("kernel").join("kernel.sqlite3");
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    connection
        .execute_batch("DROP TRIGGER chunk_sets_move_only_from_building")
        .unwrap();
    connection
        .execute(
            "UPDATE chunk_sets SET manifest_digest = NULL WHERE id = ?1",
            [chunk_set],
        )
        .unwrap();
}

/// A paragraph of `words` distinct words, `<stem>1` first, each one token
/// for the port's tokenizer.
pub(super) fn words(stem: &str, words: usize) -> String {
    (1..=words)
        .map(|number| format!("{stem}{number}"))
        .collect::<Vec<_>>()
        .join(" ")
}
