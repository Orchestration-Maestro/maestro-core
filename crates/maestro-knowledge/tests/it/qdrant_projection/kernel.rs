//! A kernel holding a complete chunk set to publish, in a scratch directory
//! removed when it is dropped: guides of three sections each, canonicalized,
//! one chunk per section and a lead chunk without one in the first guide,
//! each chunk's prepared input stored as the artifact its digest names.

use maestro_canonicalization::{CanonicalizeInput, canonicalize};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    document::{Collection, Document, Occurrence, Revision, RevisionStatus, Source},
    evidence::Span,
    scope::{Right, ScopeSet},
    store::Database,
};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// The chunks of a guide, one per section: its heading's, then `Retries`,
/// then `Logs`; the first guide also has a lead chunk without a section.
pub(super) const PER_GUIDE: usize = 3;

/// The version the even guides carry; the odd ones carry none.
pub(super) const VERSION: &str = "9.0.22";

/// What changes a guide's chunks before they are recorded, given the kernel
/// and the guide's number.
pub(super) type Change = dyn Fn(&Kernel, usize, Vec<Chunk>) -> Vec<Chunk>;

/// A scratch directory, removed with everything in it when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// A kernel with a collection of guides and their complete chunk set.
pub(super) struct Kernel {
    /// The kernel's database, closed before its directory is removed.
    pub(super) database: Database,
    /// What a principal granted the whole default workspace reads.
    pub(super) scopes: ScopeSet,
    /// The collection, a name no other test takes, in this run or another.
    pub(super) collection: String,
    /// Its complete chunk set.
    pub(super) chunk_set: String,
    /// The directory of the database, removed once the database is closed.
    _scratch: Scratch,
}

impl Kernel {
    /// A kernel whose collection holds `guides` guides, the first also
    /// occurring in the source `mirror`, and a complete chunk set of their
    /// chunks, from the first guide's.
    pub(super) fn with_guides(guides: usize) -> Self {
        Self::with_changed_guides(guides, &|_, _, chunks| chunks)
    }

    /// [`Kernel::with_guides`], each guide's chunks changed by `change`
    /// first, given the kernel and the guide's number.
    pub(super) fn with_changed_guides(guides: usize, change: &Change) -> Self {
        let kernel = Self::empty();
        kernel.record_collection();
        kernel.record_chunk_set(&kernel.chunk_set, guides, change);
        kernel
            .database
            .complete_chunk_set(&kernel.chunk_set, &kernel.manifest())
            .unwrap();
        kernel
    }

    /// Begins the chunk set `id` of the collection's first `guides` guides,
    /// and leaves it building.
    pub(super) fn begin_another(&self, id: &str, guides: usize) {
        self.record_chunk_set(id, guides, &|_, _, chunks| chunks);
    }

    /// Every chunk of the complete chunk set, in record order.
    pub(super) fn chunks(&self) -> Vec<Chunk> {
        self.database.chunks(&self.scopes, &self.chunk_set).unwrap()
    }

    /// The prepared input of `chunk`.
    pub(super) fn input(&self, chunk: &Chunk) -> String {
        String::from_utf8(self.database.get(&chunk.digest).unwrap()).unwrap()
    }

    /// The scope of the collection.
    pub(super) fn collection_scope(&self) -> String {
        format!("workspace/default/collection/{}", self.collection)
    }

    /// A kernel with nothing recorded, and a principal that reads the whole
    /// default workspace.
    fn empty() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let collection = format!(
            "it{:x}{:x}-{}",
            process::id(),
            nanos % 0xff_ffff,
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let directory = env::temp_dir().join(format!("maestro-knowledge-qdrant-{collection}"));
        fs::create_dir_all(&directory).unwrap();
        let database = Database::open_in(&directory).unwrap();
        let workspace = "workspace/default".parse().unwrap();
        database
            .grant("tester", &workspace, Right::Read, "test")
            .unwrap();
        let scopes = database.visible("tester").unwrap();
        Self {
            database,
            scopes,
            chunk_set: format!("chunk-set-{collection}"),
            collection,
            _scratch: Scratch(directory),
        }
    }

    /// Records the collection and its two sources, `docs` and `mirror`.
    fn record_collection(&self) {
        self.database
            .record_collection(&Collection {
                id: self.collection.clone(),
                title: "Guides".to_owned(),
                visibility: "private".to_owned(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        for source in ["docs", "mirror"] {
            self.database
                .record_source(&Source {
                    collection_id: self.collection.clone(),
                    id: source.to_owned(),
                    kind: "import".to_owned(),
                    transport: None,
                    reference: format!("corpus_root:{source}.jsonl"),
                    profiles: BTreeMap::new(),
                })
                .unwrap();
        }
    }

    /// Begins the chunk set `id` and records the chunks of the first `guides`
    /// guides in it, each guide's changed by `change`, recording each guide's
    /// document, revision and occurrences first.
    fn record_chunk_set(&self, id: &str, guides: usize, change: &Change) {
        self.database
            .begin_chunk_set(&NewChunkSet {
                id,
                collection_id: &self.collection,
                chunk_profile: "mapped-structural-chunks/2",
                counter_contract_id: "router/1:sha256:test",
            })
            .unwrap();
        for guide in 0..guides {
            let (revision, chunks) = self.record_guide(guide);
            let chunks = change(self, guide, chunks);
            self.database.record_chunks(id, &revision, &chunks).unwrap();
        }
    }

    /// Records the guide `guide`, its document, its revision and where it
    /// occurs, and returns its revision and its chunks.
    fn record_guide(&self, guide: usize) -> (String, Vec<Chunk>) {
        let markdown = format!(
            "# Guide {guide}\n\nGuide {guide} explains how the scheduler runs job-{guide}.\n\n\
             ## Retries\n\nJob-{guide} is retried {guide} times after it fails.\n\n\
             ## Logs\n\nThe logs of job-{guide} are kept in the artifact store.\n"
        );
        let source_ref = format!("https://example.org/guides/{guide}");
        let document = format!("doc-{}-{guide}", self.collection);
        let mut input = CanonicalizeInput::new(&markdown, &source_ref);
        input.document_id = Some(&document);
        let canonical = canonicalize(input).unwrap();
        self.database
            .record_document(&Document {
                id: document.clone(),
                collection_id: self.collection.clone(),
                source_id: "docs".to_owned(),
                source_ref: source_ref.clone(),
            })
            .unwrap();
        let mut metadata = Map::new();
        metadata.insert("source_kind".to_owned(), json!("guide"));
        if guide.is_multiple_of(2) {
            metadata.insert("version".to_owned(), json!(VERSION));
        }
        let revision = canonical.revision_id.clone();
        self.database
            .record_revision(&Revision {
                id: revision.clone(),
                document_id: document,
                original_digest: self.put(markdown.as_bytes()),
                canonical_digest: self.put(&serde_json::to_vec(&canonical).unwrap()),
                status: RevisionStatus::Valid,
                captured_at: None,
                metadata,
            })
            .unwrap();
        self.record_places(guide, &revision, &source_ref);
        let mut chunks: Vec<Chunk> = canonical
            .sections
            .iter()
            .enumerate()
            .map(|(place, section)| {
                let text = format!(
                    "{}\n\nPart {place} of guide {guide}: the scheduler runs job-{guide} and \
                     retries it.",
                    section.heading_path.join(" > ")
                );
                self.chunk(
                    &revision,
                    format!("chunk-{guide}-{place}"),
                    Some(&section.section_id),
                    &text,
                )
            })
            .collect();
        assert_eq!(chunks.len(), PER_GUIDE);
        if guide == 0 {
            let lead = "Guides of the scheduler, in French and English.";
            chunks.insert(
                0,
                self.chunk(&revision, "chunk-0-lead".to_owned(), None, lead),
            );
        }
        (revision, chunks)
    }

    /// Records where the content of `revision`, of the guide `guide` from
    /// `source_ref`, occurs: in `docs`, and for the first guide in `mirror`
    /// too.
    fn record_places(&self, guide: usize, revision: &str, source_ref: &str) {
        let mut places = vec![Occurrence {
            revision_id: revision.to_owned(),
            collection_id: self.collection.clone(),
            source_id: "docs".to_owned(),
            source_ref: source_ref.to_owned(),
        }];
        if guide == 0 {
            places.push(Occurrence {
                source_id: "mirror".to_owned(),
                source_ref: "https://mirror.example.org/guides/0".to_owned(),
                ..places[0].clone()
            });
        }
        self.database.record_occurrences(&places).unwrap();
    }

    /// The chunk `id` of `revision`, in `section` if any, whose prepared
    /// input is `text`, stored.
    fn chunk(&self, revision: &str, id: String, section: Option<&str>, text: &str) -> Chunk {
        Chunk {
            id,
            revision_id: revision.to_owned(),
            section_id: section.map(str::to_owned),
            digest: self.put(text.as_bytes()),
            token_count: text.split_whitespace().count() as u64,
            span: Span { start: 0, end: 1 },
        }
    }

    /// Stores `bytes` as an artifact of the kernel.
    pub(super) fn put(&self, bytes: &[u8]) -> Digest {
        self.database
            .put(bytes, "text/plain; charset=utf-8")
            .unwrap()
    }

    /// A manifest to complete a chunk set with.
    fn manifest(&self) -> Digest {
        let manifest: Value = json!({ "schema": "maestro-chunk-set/1", "test": true });
        self.database
            .put(manifest.to_string().as_bytes(), "application/json")
            .unwrap()
    }
}
