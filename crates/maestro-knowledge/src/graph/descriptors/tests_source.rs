//! Real kernel authority survives disposable descriptor deletion.

use super::{
    DescriptorInput, DescriptorPin, build,
    tests::{MARKDOWN, fixture},
};
use crate::graph::{
    build::inputs,
    resolve::{EXACT_RESOLVER_VERSION, validate_snapshot},
    verify::Source as VerifiedSource,
};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    document::{Collection, Disposition, Document, Outcome, Revision, RevisionStatus, Source},
    evidence::Span,
    facts::{Batch, Budget, BuildPlan, ClaimSet, ClaimSetRecord, ResolutionInput},
    generation::NewGeneration,
    job::{LeaseTiming, NewJob},
    scope::{Right, ScopeSet, WORKSPACE, collection_path},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use rusqlite::Connection;
use serde_json::{Map, json};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

#[cfg(all(feature = "engine", unix))]
use std::path::Path;

/// Kernel/artifact scratch fixture, constructed through public write APIs.
pub(crate) struct Authority {
    /// Sole source of claims and original bytes.
    pub(crate) database: Database,
    /// Source scopes before any revocation.
    pub(crate) scopes: ScopeSet,
    /// Frozen generation view.
    pub(crate) pin: DescriptorPin,
    /// Frozen reviewed identity snapshot.
    pub(super) resolution: Digest,
    /// Last field: remove the directory only after the database closes.
    path: Directory,
}

/// A scratch directory removed after the authority's database drops.
struct Directory(PathBuf);

impl Authority {
    /// Held kernel scratch path for re-executed crate-internal process tests.
    #[cfg(all(feature = "engine", unix))]
    pub(crate) fn directory(&self) -> &Path {
        &self.path.0
    }

    /// Populate a real attached generation and admitted synthetic claims.
    pub(crate) fn new() -> Self {
        Self::with_chunks(true, true)
    }

    /// Build a real generation with explicit complete/source-membership variants.
    fn with_chunks(complete: bool, include_revision: bool) -> Self {
        let path = scratch_directory().unwrap();
        let database = Database::open_in(&path).unwrap();
        let input = fixture();
        let source = input.sources.values().next().unwrap();
        populate(&database, source, (complete, include_revision));
        database
            .grant("builder", &WORKSPACE.parse().unwrap(), Right::Read, "test")
            .unwrap();
        let scopes = database.visible("builder").unwrap();
        let (generation, set) = freeze_and_attach(&database, &scopes, &input);
        let snapshot = database
            .record_resolution(
                &scopes,
                "builder",
                &ResolutionInput {
                    resolver_version: EXACT_RESOLVER_VERSION.into(),
                    sets: vec![set.id],
                    previous: None,
                    decisions: vec![],
                },
                &validate_snapshot,
            )
            .unwrap();
        Self {
            database,
            scopes,
            pin: DescriptorPin {
                collection_id: "graph".into(),
                generation_id: generation,
                version: None,
            },
            resolution: snapshot.id,
            path: Directory(path),
        }
    }

    /// Reopen the same authority view without consulting any vector storage.
    pub(super) fn read(&self) -> DescriptorInput {
        DescriptorInput::read(
            &self.database,
            &self.scopes,
            "builder",
            (&self.pin, &self.resolution),
        )
        .unwrap()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Build and attach the admitted claims using real fenced kernel jobs.
fn freeze_and_attach(
    database: &Database,
    scopes: &ScopeSet,
    input: &DescriptorInput,
) -> (i64, ClaimSetRecord) {
    let scope = collection_path("graph").parse().unwrap();
    let plan = BuildPlan {
        collection_id: "graph".into(),
        provenance: input.claims[0].claim.provenance.clone(),
        sources: input.sources.keys().cloned().collect(),
        budget: Budget {
            max_claims: 4,
            max_rejections: 4,
        },
    };
    let now = SystemTime::now();
    let timing = LeaseTiming {
        now,
        term: Duration::from_secs(600),
    };
    let body = inputs(&plan, None);
    let job = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.build",
                inputs: &body,
                scope: &scope,
                resource: None,
            },
            now,
        )
        .unwrap();
    let mut lease = database.take_job(job.id, "test", now, timing.term).unwrap();
    database.begin_graph_build(scopes, &lease, &plan).unwrap();
    database
        .record_graph_batch(
            scopes,
            &mut lease,
            timing,
            &Batch {
                ordinal: 0,
                claims: input
                    .claims
                    .iter()
                    .map(|record| record.claim.clone())
                    .collect(),
                rejections: vec![],
            },
        )
        .unwrap();
    let set = database
        .finish_graph_build(scopes, &mut lease, timing)
        .unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "graph".into(),
            chunk_set_id: "chunks".into(),
            embedding_profile: "unused".into(),
            sparse_profile: "unused".into(),
        })
        .unwrap();
    let body = json!({"build": job.id.to_string(), "generation": generation.id});
    let attachment_job = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.attach",
                inputs: &body,
                scope: &scope,
                resource: None,
            },
            now,
        )
        .unwrap();
    let lease = database
        .take_job(attachment_job.id, "test", now, timing.term)
        .unwrap();
    database
        .attach_claim_set(scopes, generation.id, job.id, &lease)
        .unwrap();
    (generation.id, set)
}

/// Record the original/canonical artifacts, disposition and exact chunk membership.
fn populate(database: &Database, source: &VerifiedSource, membership: (bool, bool)) {
    database
        .record_collection(&Collection {
            id: "graph".into(),
            title: "Synthetic graph".into(),
            visibility: "public".into(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .record_source(&Source {
            collection_id: "graph".into(),
            id: "docs".into(),
            kind: "import".into(),
            transport: None,
            reference: "synthetic:descriptors".into(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .record_document(&Document {
            id: "doc".into(),
            collection_id: "graph".into(),
            source_id: "docs".into(),
            source_ref: "synthetic:descriptors".into(),
        })
        .unwrap();
    database
        .record_revision(&Revision {
            id: source.revision_id().into(),
            document_id: "doc".into(),
            original_digest: database.put(MARKDOWN.as_bytes(), "text/markdown").unwrap(),
            canonical_digest: database
                .put(
                    &serde_json::to_vec(source.canonical()).unwrap(),
                    "application/json",
                )
                .unwrap(),
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::from_iter([("version".into(), json!("1.0"))]),
        })
        .unwrap();
    database
        .record_disposition(&Disposition {
            revision_id: source.revision_id().into(),
            outcome: Outcome::Accepted,
            reasons: vec![],
            rule_ids: vec![],
            decided_by: "test".into(),
        })
        .unwrap();
    database
        .begin_chunk_set(&NewChunkSet {
            id: "chunks",
            collection_id: "graph",
            chunk_profile: "synthetic/1",
            counter_contract_id: "synthetic/1",
        })
        .unwrap();
    if membership.1 {
        database
            .record_chunks(
                "chunks",
                source.revision_id(),
                &[Chunk {
                    id: "chunk".into(),
                    revision_id: source.revision_id().into(),
                    section_id: None,
                    digest: database.put(MARKDOWN.as_bytes(), "text/markdown").unwrap(),
                    token_count: 12,
                    span: Span {
                        start: 0,
                        end: MARKDOWN.len(),
                    },
                }],
            )
            .unwrap();
    }
    if membership.0 {
        let manifest = database.put(b"{}", "application/json").unwrap();
        database.complete_chunk_set("chunks", &manifest).unwrap();
    }
}

#[test]
fn authority_closes_the_database_before_removing_its_directory() {
    let fixture = Authority::new();
    let path = fixture.path.0.clone();
    assert!(path.join("kernel.sqlite3").is_file());
    drop(fixture);
    assert!(!path.exists());
}

#[test]
fn resolution_must_include_the_generation_attachment() {
    let fixture = Authority::new();
    let mut claim = fixture.read().claims[0].claim.clone();
    claim.conditions.insert("mode".into(), "alternate".into());
    let other = fixture
        .database
        .record_claim_set(
            &fixture.scopes,
            &ClaimSet {
                collection_id: "graph".into(),
                claims: vec![claim],
            },
        )
        .unwrap();
    let snapshot = fixture
        .database
        .record_resolution(
            &fixture.scopes,
            "builder",
            &ResolutionInput {
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                sets: vec![other.id],
                previous: None,
                decisions: vec![],
            },
            &validate_snapshot,
        )
        .unwrap();
    assert!(
        DescriptorInput::read(
            &fixture.database,
            &fixture.scopes,
            "builder",
            (&fixture.pin, &snapshot.id)
        )
        .is_err()
    );
}

#[test]
fn reader_refreshes_current_grants_despite_a_stale_request() {
    let fixture = Authority::new();
    fixture
        .database
        .revoke("builder", &WORKSPACE.parse().unwrap(), Right::Read, "test")
        .unwrap();
    assert!(
        DescriptorInput::read(
            &fixture.database,
            &fixture.scopes,
            "builder",
            (&fixture.pin, &fixture.resolution)
        )
        .is_err()
    );
}

#[test]
fn reader_refuses_failed_incomplete_misbound_or_ineligible_sources() {
    for (complete, include_revision) in [(false, true), (true, false)] {
        let fixture = Authority::with_chunks(complete, include_revision);
        assert!(
            DescriptorInput::read(
                &fixture.database,
                &fixture.scopes,
                "builder",
                (&fixture.pin, &fixture.resolution)
            )
            .is_err()
        );
    }
    for statement in [
        "UPDATE generations SET state = 'failed'",
        "UPDATE revisions SET status = 'failed'",
        "UPDATE quality_dispositions SET disposition = 'quarantined'",
    ] {
        let fixture = Authority::new();
        Connection::open(fixture.path.0.join("kernel.sqlite3"))
            .unwrap()
            .execute(statement, [])
            .unwrap();
        let result = DescriptorInput::read(
            &fixture.database,
            &fixture.scopes,
            "builder",
            (&fixture.pin, &fixture.resolution),
        );
        assert!(result.is_err(), "accepted {statement}");
    }
}

#[test]
fn reader_uses_real_attached_authority_and_original_artifacts() {
    let fixture = Authority::new();
    let descriptors = build(&fixture.read()).unwrap();
    assert_eq!(descriptors.len(), 3);
    assert!(
        descriptors
            .iter()
            .any(|item| item.text.contains("REQUIRES"))
    );
}

#[test]
fn reader_refuses_wrong_generation_wrong_version_and_hidden_resolution() {
    let fixture = Authority::new();
    let mut pin = fixture.pin.clone();
    pin.generation_id += 1;
    assert!(
        DescriptorInput::read(
            &fixture.database,
            &fixture.scopes,
            "builder",
            (&pin, &fixture.resolution)
        )
        .is_err()
    );
    pin = fixture.pin.clone();
    pin.collection_id = "other".into();
    assert!(
        DescriptorInput::read(
            &fixture.database,
            &fixture.scopes,
            "builder",
            (&pin, &fixture.resolution)
        )
        .is_err()
    );
    pin = fixture.pin.clone();
    pin.version = Some("2.0".into());
    assert!(
        DescriptorInput::read(
            &fixture.database,
            &fixture.scopes,
            "builder",
            (&pin, &fixture.resolution)
        )
        .is_err()
    );
    assert!(
        DescriptorInput::read(
            &fixture.database,
            &fixture.scopes,
            "unknown",
            (&fixture.pin, &fixture.resolution)
        )
        .is_err()
    );
}
