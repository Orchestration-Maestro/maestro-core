//! A synthetic authoritative literal claim built entirely through public kernel APIs.
#![cfg(test)]
use maestro_canonicalization::{CanonicalizeInput, canonicalize};
use maestro_filesystem::{ControlFile, OwnedRoot, SystemFileLock};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    document::{Collection, Disposition, Document, Outcome, Revision, RevisionStatus, Source},
    evidence::Span,
    facts::{
        Batch, Budget, BuildPlan, Claim, EntityKind, EntityName, Literal, LiteralKind, Object,
        Predicate, Provenance, Support, Validity,
    },
    generation::NewGeneration,
    job::{Lease, LeaseTiming, NewJob},
    scope::{Right, collection_path},
    store::Database,
};
use maestro_knowledge::graph::projection::{
    EngineSettings, ProjectionBuild, ProjectionEngine, ProjectionFactory, ProjectionScope,
};
use maestro_test_scratch::scratch_directory;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

pub(super) struct Fixture {
    pub(super) kernel: Database,
    pub(super) build: ProjectionBuild,
    pub(super) now: SystemTime,
    directory: Scratch,
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

pub(super) fn settings() -> EngineSettings {
    EngineSettings::new(
        16 * 1024 * 1024,
        64 * 1024 * 1024,
        1,
        Digest::of(b"process frozen lock"),
    )
    .unwrap()
}
pub(super) fn factory(path: &Path) -> ProjectionFactory<'static> {
    ProjectionFactory::new(path, ProjectionEngine::Ladybug, settings(), &SystemFileLock)
}

impl Fixture {
    pub(super) fn new() -> Self {
        let directory = Scratch(scratch_directory().unwrap());
        let kernel = Database::open_in(&directory.0).unwrap();
        kernel
            .grant(
                "lifecycle",
                &"workspace/default".parse().unwrap(),
                Right::Read,
                "test",
            )
            .unwrap();
        let scopes = kernel.visible("lifecycle").unwrap();
        let claim = source(&kernel);
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let timing = LeaseTiming {
            now,
            term: Duration::from_secs(60),
        };
        let plan = BuildPlan {
            collection_id: "graph".into(),
            provenance: claim.provenance.clone(),
            sources: vec![claim.supports[0].revision_id.clone()],
            budget: Budget {
                max_claims: 1,
                max_rejections: 0,
            },
        };
        let mut builder = lease(
            &kernel,
            "knowledge.graph.build",
            &json!({"fixture":"process"}),
            timing,
        );
        kernel.begin_graph_build(&scopes, &builder, &plan).unwrap();
        kernel
            .record_graph_batch(
                &scopes,
                &mut builder,
                timing,
                &Batch {
                    ordinal: 0,
                    claims: vec![claim],
                    rejections: vec![],
                },
            )
            .unwrap();
        let set = kernel
            .finish_graph_build(&scopes, &mut builder, timing)
            .unwrap();
        let generation = kernel
            .create_generation(&NewGeneration {
                collection_id: "graph".into(),
                chunk_set_id: "chunks".into(),
                embedding_profile: "synthetic".into(),
                sparse_profile: "synthetic".into(),
            })
            .unwrap()
            .id;
        let attach = lease(
            &kernel,
            "knowledge.graph.attach",
            &json!({"build":builder.job.to_string(),"generation":generation}),
            timing,
        );
        kernel
            .attach_claim_set(&scopes, generation, builder.job, &attach)
            .unwrap();
        kernel.verify_generation(generation, 1).unwrap();
        let projector = lease(
            &kernel,
            "knowledge.graph.project",
            &json!({"generation":generation}),
            timing,
        );
        let scope = ProjectionScope {
            collection_id: "graph".into(),
            generation_id: generation,
        };
        let root = OwnedRoot::open(&directory.0.join("graph"), true).unwrap();
        root.ensure_control(ControlFile::Access).unwrap();
        root.ensure_control(ControlFile::Writer).unwrap();
        Self {
            kernel,
            now,
            build: ProjectionBuild {
                scope,
                claim_set_id: set.id,
                lease: projector,
            },
            directory,
        }
    }
    pub(super) fn path(&self) -> &Path {
        &self.directory.0
    }
    pub(super) fn graph(&self) -> PathBuf {
        self.directory.0.join("graph")
    }
}

pub(super) fn lease(kernel: &Database, kind: &str, inputs: &Value, timing: LeaseTiming) -> Lease {
    let scope = collection_path("graph").parse().unwrap();
    let job = kernel
        .submit_job(
            &NewJob {
                kind,
                inputs,
                scope: &scope,
                resource: None,
            },
            timing.now,
        )
        .unwrap();
    kernel
        .take_job(job.id, "process", timing.now, timing.term)
        .unwrap()
}

fn source(kernel: &Database) -> Claim {
    let text = "mode defaults to fast.\n";
    let canonical = canonicalize(CanonicalizeInput::new(text, "synthetic:lifecycle")).unwrap();
    let original = kernel.put(text.as_bytes(), "text/markdown").unwrap();
    record_source(kernel);
    kernel
        .record_revision(&Revision {
            id: canonical.revision_id.clone(),
            document_id: "document".into(),
            original_digest: original.clone(),
            canonical_digest: kernel
                .put(&serde_json::to_vec(&canonical).unwrap(), "application/json")
                .unwrap(),
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::new(),
        })
        .unwrap();
    kernel
        .record_disposition(&Disposition {
            revision_id: canonical.revision_id.clone(),
            outcome: Outcome::Accepted,
            reasons: vec![],
            rule_ids: vec![],
            decided_by: "test".into(),
        })
        .unwrap();
    kernel
        .begin_chunk_set(&NewChunkSet {
            id: "chunks",
            collection_id: "graph",
            chunk_profile: "synthetic/1",
            counter_contract_id: "synthetic/1",
        })
        .unwrap();
    kernel
        .record_chunks(
            "chunks",
            &canonical.revision_id,
            &[Chunk {
                id: "chunk".into(),
                revision_id: canonical.revision_id.clone(),
                section_id: None,
                digest: original,
                token_count: 4,
                span: Span {
                    start: 0,
                    end: text.len(),
                },
            }],
        )
        .unwrap();
    let manifest = kernel.put(b"{}", "application/json").unwrap();
    kernel.complete_chunk_set("chunks", &manifest).unwrap();
    let span = Span {
        start: 0,
        end: text.len() - 1,
    };
    let block = canonical
        .blocks
        .iter()
        .find(|block| {
            block
                .source_spans
                .iter()
                .any(|outer| outer.start <= span.start && span.end <= outer.end)
        })
        .unwrap();
    Claim {
        subject: EntityName {
            kind: EntityKind::Parameter,
            name: "mode".into(),
        },
        predicate: Predicate::DefaultsTo,
        object: Object::Literal(Literal {
            kind: LiteralKind::Text,
            lexeme: "fast".into(),
        }),
        conditions: BTreeMap::new(),
        version: Validity::Unknown,
        world: Validity::Unknown,
        provenance: Provenance {
            extractor: "synthetic/1".into(),
            profile: Digest::of(b"profile"),
        },
        supports: vec![Support {
            revision_id: canonical.revision_id.clone(),
            block_id: block.block_id.clone(),
            span,
            quote_digest: Digest::of(&text.as_bytes()[..span.end]),
        }],
    }
}

fn record_source(kernel: &Database) {
    kernel
        .record_collection(&Collection {
            id: "graph".into(),
            title: "Synthetic".into(),
            visibility: "private".into(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    kernel
        .record_source(&Source {
            collection_id: "graph".into(),
            id: "source".into(),
            kind: "import".into(),
            transport: None,
            reference: "synthetic:lifecycle".into(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    kernel
        .record_document(&Document {
            id: "document".into(),
            collection_id: "graph".into(),
            source_id: "source".into(),
            source_ref: "synthetic:lifecycle".into(),
        })
        .unwrap();
}
