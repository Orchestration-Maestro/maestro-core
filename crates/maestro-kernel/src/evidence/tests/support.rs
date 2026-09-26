//! What the evidence tests share: a scratch database holding one revision of
//! a document with its original Markdown and a chunk set cut from it, and a
//! bundle that cites it.

use crate::{
    artifact::Digest,
    document::{Collection, Document, Revision, RevisionStatus, Source},
    evidence::{Alternate, Budget, Bundle, Conflict, Passage, RouteStatus, Schema, Span, Trace},
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

/// The original Markdown of the revision `rev-a`: two sections that state
/// different default ports, the second with a dash that takes three bytes.
pub(super) const ORIGINAL: &str = "# Installing the agent\n\n## Prerequisites\n\n\
                                   The agent listens on port 7005 by default.\n\n\
                                   ## Ports\n\n\
                                   The default port is 7006 — unless the installer finds it \
                                   taken.\n";

/// The origin of the document `doc-a`.
pub(super) const SOURCE_REF: &str = "https://example.org/agent/install";

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: after the database a test opened in
/// it, which it declares later.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-evidence-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The kernel database of this directory, with the revision `rev-a` of
    /// the document `doc-a` recorded with `metadata`, its original Markdown
    /// [`ORIGINAL`], and the chunk set `set-a` of its collection, which holds
    /// no chunk yet.
    pub(super) fn open(&self, metadata: Map<String, Value>) -> Database {
        let database = Database::open_in(&self.0).unwrap();
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
                source_ref: SOURCE_REF.to_owned(),
            })
            .unwrap();
        let revision = Revision {
            id: "rev-a".to_owned(),
            document_id: "doc-a".to_owned(),
            original_digest: database.put(ORIGINAL.as_bytes(), "text/markdown").unwrap(),
            canonical_digest: database.put(b"{}", "application/json").unwrap(),
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata,
        };
        database.record_revision(&revision).unwrap();
        execute(
            &database,
            "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state)
             VALUES ('set-a', 'ctm', 'structural-500-700/1', 'native', 'complete')",
        );
        database
    }

    /// Where the artifact store of this directory's database keeps `digest`.
    pub(super) fn stored(&self, digest: &Digest) -> PathBuf {
        let hex = digest.as_str();
        self.0
            .join("artifacts")
            .join("sha256")
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(hex)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// The metadata of a revision titled `Installing the agent`, with `version`
/// if there is one.
pub(super) fn metadata(version: Option<Value>) -> Map<String, Value> {
    let mut metadata = Map::from_iter([("title".to_owned(), Value::from("Installing the agent"))]);
    if let Some(version) = version {
        metadata.insert("version".to_owned(), version);
    }
    metadata
}

/// Records the chunk `id` of the chunk set `set-a` over `span` of the
/// revision `rev-a`, in the section `section` if it has one. Its digest
/// hashes its prepared input, the heading context before its text, so it is
/// never the digest of the source text its span covers.
pub(super) fn chunk(database: &Database, id: &str, section: Option<&str>, span: Span) {
    let prepared = format!("Installing the agent > {id}");
    database
        .write(|transaction| {
            transaction.execute(
                "INSERT INTO chunks (chunk_set_id, id, revision_id, section_id, digest,
                   token_count, span_start, span_end)
                 VALUES ('set-a', ?1, 'rev-a', ?2, ?3, 12, ?4, ?5)",
                rusqlite::params![
                    id,
                    section,
                    Digest::of(prepared.as_bytes()).as_str(),
                    i64::try_from(span.start).unwrap(),
                    i64::try_from(span.end).unwrap(),
                ],
            )?;
            Ok::<_, store::Error>(())
        })
        .unwrap();
}

/// The span of `ORIGINAL` that `text` takes, where it first appears.
pub(super) fn span_of(text: &str) -> Span {
    let start = ORIGINAL.find(text).unwrap();
    Span {
        start,
        end: start + text.len(),
    }
}

/// Runs `statement` on the writer and commits what it did.
fn execute(database: &Database, statement: &str) {
    database
        .write(|transaction| {
            transaction
                .execute(statement, [])
                .map_err(store::Error::from)
        })
        .unwrap();
}

/// A bundle that cites two passages of `rev-a`, the second with no version
/// and no section, which differ on a default port; its identifier route
/// could not run, and it knows of a gap.
pub(super) fn bundle() -> Bundle {
    let first = "The agent listens on port 7005 by default.";
    let second = "The default port is 7006 — unless the installer finds it taken.";
    Bundle {
        schema: Schema::V1,
        collection: "ctm".to_owned(),
        generation: 7,
        query: "Which port does the agent listen on?".to_owned(),
        lang: "en".to_owned(),
        routes: BTreeMap::from([
            ("bm25".to_owned(), RouteStatus::Ok),
            ("dense".to_owned(), RouteStatus::Ok),
            (
                "identifier".to_owned(),
                RouteStatus::Unavailable("timed out after 400 ms".to_owned()),
            ),
            ("rerank".to_owned(), RouteStatus::Ok),
        ]),
        passages: vec![
            Passage {
                n: 1,
                section_id: Some("sec-prerequisites".to_owned()),
                document_id: "doc-a".to_owned(),
                revision_id: "rev-a".to_owned(),
                title: "Installing the agent".to_owned(),
                section_path: vec![
                    "Installing the agent".to_owned(),
                    "Prerequisites".to_owned(),
                ],
                version: Some("2.1.0".to_owned()),
                source_ref: SOURCE_REF.to_owned(),
                span: span_of(first),
                digest: Digest::of(first.as_bytes()),
                text: first.to_owned(),
                alternates: vec![Alternate {
                    version: Some("2.0.0".to_owned()),
                    section_id: "sec-prerequisites-2-0".to_owned(),
                }],
            },
            Passage {
                n: 2,
                section_id: None,
                document_id: "doc-a".to_owned(),
                revision_id: "rev-a".to_owned(),
                title: "Installing the agent".to_owned(),
                section_path: Vec::new(),
                version: None,
                source_ref: SOURCE_REF.to_owned(),
                span: span_of(second),
                digest: Digest::of(second.as_bytes()),
                text: second.to_owned(),
                alternates: Vec::new(),
            },
        ],
        conflicts: vec![Conflict {
            entity: "agent".to_owned(),
            attribute: "default port".to_owned(),
            passages: vec![1, 2],
        }],
        known_gaps: vec!["no passage states the port of version 2.0.0".to_owned()],
        budget: Budget {
            evidence_tokens: 41,
            limit: 6000,
        },
        trace: vec![
            Trace {
                n: 1,
                score: Some(0.83),
                routes: vec!["bm25".to_owned(), "dense".to_owned()],
                procedural: false,
            },
            Trace {
                n: 2,
                score: Some(0.41),
                routes: vec!["dense".to_owned()],
                procedural: true,
            },
        ],
    }
}
