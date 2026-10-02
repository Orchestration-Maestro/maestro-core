//! N26 shared synthetic capture and stored-canonical fixtures.
#![cfg(test)]
use maestro_canonicalization::{
    AssetStatus, CanonicalDocument, ExtractorBlock, OriginalLocation, SourceMetadata, SourceSpan,
};
use maestro_kernel::{
    acquisition::{
        CaptureContext, CaptureEnvelope, Captures, DispatchRequest, Frontier, Handle, LeaseRequest,
        NewItem, Receipts, Representation, SafeIdentity, Transport,
    },
    artifact::Digest,
    document::Revision,
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use maestro_knowledge::{
    collection::Declaration,
    import::{self, AssetRecord, MappedEvidence, MappedInput, Target},
};
use maestro_test_scratch::scratch_directory;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

/// Faithful synthetic PDF table and image output.
pub(super) const MARKDOWN: &str =
    "# Manual\n\n| Key | Value |\n| --- | --- |\n| x | 7 |\n\n![Diagram](diagram.png)\n";
/// Exact legacy spelling, not a normalized URL.
pub(super) const SOURCE_REF: &str = "https://example.org/manual.pdf?b=2&a=1#Page";
/// Frozen corpus/1 identity on a24b35d.
pub(super) const DOCUMENT_ID: &str =
    "doc-259ad0ae52ed0f5219416cf99b19e47fd9ce0d25e4e34abd487878260ac453f2";
/// Frozen corpus/1 revision identity on a24b35d.
pub(super) const CORPUS_REVISION: &str =
    "rev-09c1cafc021e58a702920f0636e80ef296013e4421854ac43a806a2d441fd88f";
/// The core's reserved identity binding.
pub(super) const ASSETS: &str = "maestro.native_assets/1";

/// Independent kernel and manifest storage for each test.
pub(super) struct Fixture {
    /// Database drops before its directory.
    pub(super) db: Database,
    /// Current read grants.
    pub(super) scopes: ScopeSet,
    /// Raw-media and fidelity evidence, outside revision metadata.
    pub(super) evidence: MappedEvidence,
    /// Current N12 lease used only for an equal recapture fixture.
    capture_context: CaptureContext,
    /// Owned scratch directory.
    pub(super) root: PathBuf,
}
impl Fixture {
    /// Declares a source and retains synthetic raw PDF and fidelity evidence.
    pub(super) fn new() -> Self {
        let root = scratch_directory().unwrap();
        let db = Database::open_in(&root.join("kernel")).unwrap();
        db.grant(
            "tester",
            &"workspace/default".parse().unwrap(),
            Right::Read,
            "test",
        )
        .unwrap();
        let scopes = db.visible("tester").unwrap();
        import::declare(&db, &scopes, &declaration()).unwrap();
        let scope: Scope = "workspace/default/collection/manuals/source/docs"
            .parse()
            .unwrap();
        let raw = b"%PDF-synthetic raw table and image";
        let artifact = db.put(raw, "application/pdf").unwrap();
        let context = db.retain(&scope, b"synthetic context", &[]).unwrap();
        let now = SystemTime::now();
        let lease = LeaseRequest {
            holder: "worker",
            now,
            term: Duration::from_mins(5),
        };
        let writer = db.lease_source("docs", &scope, lease).unwrap();
        let item = db
            .enqueue(
                &writer,
                &NewItem {
                    fetch_identity: SOURCE_REF.into(),
                    authorization_context: Digest::of(b"public"),
                    representation_profile: Digest::of(b"pdf/1"),
                },
                now,
            )
            .unwrap();
        let dispatch = db
            .lease(
                &writer,
                item.id,
                DispatchRequest {
                    lease,
                    max_attempts: 1,
                },
            )
            .unwrap();
        let capture_context = CaptureContext {
            writer,
            item: dispatch,
            now,
        };
        let envelope = CaptureEnvelope {
            schema: "maestro-capture/1".into(),
            source: "docs".into(),
            item: item.id.to_string().parse().unwrap(),
            run: Handle::new(),
            requested: SafeIdentity::new(SOURCE_REF).unwrap(),
            final_identity: SafeIdentity::new(SOURCE_REF).unwrap(),
            redirects: vec![],
            status: 200,
            headers: BTreeMap::new(),
            declared_media: Some("application/pdf".into()),
            detected_media: Some("application/pdf".into()),
            artifact,
            length: raw.len() as u64,
            observed_ms: 1,
            transport: Transport::Http,
            profile: Digest::of(b"pdf/1"),
            authorization_context: Digest::of(b"public"),
            representation: Representation::WireBody,
            parent: None,
            inputs: context,
            access: context,
            decision: context,
        };
        let capture = db
            .prepare_capture(&capture_context, &envelope, raw, u64::MAX)
            .unwrap()
            .handle;
        db.acknowledge_capture(&capture_context, capture).unwrap();
        let fidelity = db
            .retain(&scope, b"synthetic fidelity receipt", &[capture])
            .unwrap();
        Self {
            db,
            scopes,
            evidence: MappedEvidence { capture, fidelity },
            capture_context,
            root,
        }
    }
    /// The same declared collection/source for both ingestion routes.
    pub(super) fn target(&self) -> Target<'_> {
        Target {
            database: &self.db,
            scopes: &self.scopes,
            collection: "manuals",
            source: "docs",
        }
    }
    /// Faithful mappings, with a supplied PDF cell and an explicitly unknown coordinate.
    pub(super) fn input(&self) -> MappedInput<'_> {
        let start = MARKDOWN.find("| Key").unwrap();
        MappedInput {
            markdown: MARKDOWN.as_bytes(),
            digest: Digest::of(MARKDOWN.as_bytes()),
            length: MARKDOWN.len() as u64,
            source_ref: SOURCE_REF,
            metadata: SourceMetadata {
                source_reference: Some(SOURCE_REF.into()),
                title: Some("Manual".into()),
                extra: BTreeMap::from([("source_kind".into(), json!("guide"))]),
                ..SourceMetadata::default()
            },
            revision_metadata: serde_json::from_value(
                json!({"title":"Manual","source_kind":"guide"}),
            )
            .unwrap(),
            captured_at: None,
            operational_metadata: BTreeMap::new(),
            extractor_blocks: vec![
                ExtractorBlock {
                    extractor_id: "table-1".into(),
                    markdown_spans: vec![SourceSpan {
                        start,
                        end: start + 43,
                    }],
                    original_locations: vec![OriginalLocation {
                        source_reference: Some(SOURCE_REF.into()),
                        page: Some(3),
                        locator: Some(json!({"cell":"A2","region":[1,2,3,4]})),
                    }],
                    structured_content: json!({"rows":[["Key","Value"],["x","7"]]}),
                },
                ExtractorBlock {
                    extractor_id: "image-1".into(),
                    markdown_spans: vec![SourceSpan {
                        start: MARKDOWN.find("![").unwrap(),
                        end: MARKDOWN.len() - 1,
                    }],
                    original_locations: vec![],
                    structured_content: json!({"asset":"diagram.png"}),
                },
            ],
            assets: vec![AssetRecord {
                destination: "diagram.png".into(),
                status: AssetStatus::Missing,
                digest: None,
                length: None,
            }],
            disposition: None,
            evidence: self.evidence.clone(),
        }
    }
    /// Prepares another acknowledged N12 capture under the same source without changing content.
    pub(super) fn recapture(&self) -> MappedEvidence {
        let mut envelope: CaptureEnvelope = serde_json::from_slice(
            self.db
                .read("tester", self.evidence.capture)
                .unwrap()
                .unwrap()
                .bytes(),
        )
        .unwrap();
        envelope.authorization_context = Digest::of(b"public-recapture");
        let lease = LeaseRequest {
            holder: "worker",
            now: self.capture_context.now,
            term: Duration::from_mins(5),
        };
        let item = self
            .db
            .enqueue(
                &self.capture_context.writer,
                &NewItem {
                    fetch_identity: SOURCE_REF.into(),
                    authorization_context: envelope.authorization_context.clone(),
                    representation_profile: envelope.profile.clone(),
                },
                lease.now,
            )
            .unwrap();
        let dispatch = self
            .db
            .lease(
                &self.capture_context.writer,
                item.id,
                DispatchRequest {
                    lease,
                    max_attempts: 1,
                },
            )
            .unwrap();
        let context = CaptureContext {
            writer: self.capture_context.writer.clone(),
            item: dispatch,
            now: lease.now,
        };
        envelope.item = item.id.to_string().parse().unwrap();
        envelope.run = Handle::new();
        envelope.observed_ms += 1;
        let capture = self
            .db
            .prepare_capture(
                &context,
                &envelope,
                &self.db.get(&envelope.artifact).unwrap(),
                u64::MAX,
            )
            .unwrap()
            .handle;
        self.db.acknowledge_capture(&context, capture).unwrap();
        let scope: Scope = "workspace/default/collection/manuals/source/docs"
            .parse()
            .unwrap();
        let fidelity = self
            .db
            .retain(&scope, b"equal recapture fidelity", &[capture])
            .unwrap();
        MappedEvidence { capture, fidelity }
    }
    /// Reads every immutable revision, including held ones.
    pub(super) fn revisions(&self) -> Vec<Revision> {
        self.db.revisions(&self.scopes, "manuals").unwrap()
    }
    /// Resolves the stored canonical artifact, not an in-memory extraction.
    pub(super) fn canonical(&self, revision: &Revision) -> CanonicalDocument {
        serde_json::from_slice(&self.db.get(&revision.canonical_digest).unwrap()).unwrap()
    }
    /// Adds verified available bytes to the shared artifact store.
    pub(super) fn available(&self, bytes: &[u8]) -> AssetRecord {
        AssetRecord {
            destination: "diagram.png".into(),
            status: AssetStatus::Available,
            digest: Some(self.db.put(bytes, "image/png").unwrap()),
            length: Some(bytes.len() as u64),
        }
    }
    /// External SQL connection for immutability and rollback assertions.
    pub(super) fn sql(&self) -> Connection {
        Connection::open(self.root.join("kernel/kernel.sqlite3")).unwrap()
    }
    /// Corrupts only this fixture's generated artifact bytes.
    pub(super) fn corrupt(&self, digest: &Digest) {
        let hex = digest.as_str();
        let path = self
            .root
            .join("kernel/artifacts/sha256")
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(hex);
        fs::write(path, b"corrupted bytes").unwrap();
    }
    /// Runs the unchanged corpus/1 route with this same exact Markdown and reference.
    pub(super) fn corpus(&self, lines: &[Value]) -> import::Report {
        fs::write(self.root.join("manual.md"), MARKDOWN).unwrap();
        fs::write(
            self.root.join("manifest.jsonl"),
            lines
                .iter()
                .map(|line| line.to_string() + "\n")
                .collect::<String>(),
        )
        .unwrap();
        let bindings = format!("corpus_root = '{}'", self.root.display())
            .parse()
            .unwrap();
        import::import(&self.db, &self.scopes, &declaration(), &bindings).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
/// A public synthetic collection using the legacy import declaration contract.
fn declaration() -> Declaration {
    json!({
        "schema":"maestro-collection/1", "id":"manuals", "title":"Manuals", "visibility":"public",
        "profiles": { "extraction":"technical-html/1", "chunking":"structural-500-700/1",
            "embedding":"embed:winner", "sparse":"bm25-en-fr/1" },
        "quality": { "ledger":"quality/ledger.jsonl" },
        "sources": [{ "id":"docs", "kind":"import", "sync":"manual",
            "manifest": { "binding":"corpus_root", "path":"manifest.jsonl" } }],
        "evals": { "suite":"evals" }
    })
    .to_string()
    .parse()
    .unwrap()
}
/// Exactly the metadata in the pinned corpus/1 revision.
pub(super) fn line() -> Value {
    json!({ "schema":"maestro-corpus/1", "path":"manual.md",
        "sha256":Digest::of(MARKDOWN.as_bytes()).as_str(), "bytes":MARKDOWN.len(),
        "source_ref":SOURCE_REF, "title":"Manual", "source_kind":"guide" })
}

/// Tiny deterministic counter keeps this ingestion test independent of model provisioning.
struct Bytes;
impl maestro_canonicalization::TokenCounter for Bytes {
    fn contract_id(&self) -> &'static str {
        "n26/bytes"
    }
    fn verify(&self) -> Result<(), maestro_canonicalization::Error> {
        Ok(())
    }
    fn token_ids(&self, input: &str) -> Result<Vec<u32>, maestro_canonicalization::Error> {
        Ok(input.bytes().map(u32::from).collect())
    }
}
/// Chunk a stored artifact through S1, resolving every origin back into its preserved Markdown.
pub(super) fn prepared(document: &CanonicalDocument) -> Vec<String> {
    use maestro_canonicalization::{
        ChunkProfile, DedupInput, DedupScope, RevisionKey, WarningPolicy, chunk_documents,
    };
    let scope = DedupScope {
        tenant_id: "test".into(),
        workspace_id: "default".into(),
        authorized_revisions: [RevisionKey {
            document_id: document.document_id.clone(),
            revision_id: document.revision_id.clone(),
        }]
        .into_iter()
        .collect(),
    };
    let inputs = [DedupInput {
        document,
        markdown: MARKDOWN,
    }];
    let batch = chunk_documents(
        &scope,
        &inputs,
        WarningPolicy::Preserve,
        ChunkProfile::Structural,
        &Bytes,
    )
    .unwrap();
    assert!(!batch.chunks.is_empty());
    let origins: Vec<_> = batch
        .documents
        .iter()
        .flat_map(|mapped| &mapped.mapped.units)
        .flat_map(|unit| &unit.mappings)
        .flat_map(|mapping| &mapping.origins)
        .collect();
    assert!(origins.iter().all(|origin| {
        origin.span.is_valid(MARKDOWN)
            && document
                .blocks
                .iter()
                .any(|block| block.block_id == origin.block_id)
    }));
    assert!(origins.iter().any(|origin| {
        document.blocks.iter().any(|block| {
            block.block_id == origin.block_id
                && block.extractor_block_ids.contains(&"table-1".into())
        })
    }));
    batch
        .chunks
        .iter()
        .map(|chunk| chunk.retrieval_input_fingerprint.clone())
        .collect()
}
