//! Synthetic kernel authority for feature-independent cleanup tests.
use maestro_filesystem::{ControlFile, OwnedRoot};
use maestro_kernel::facts::EXACT_RESOLVER_VERSION;
use maestro_kernel::facts::ResolutionInput;
use maestro_kernel::facts::{ProjectionBuildRequest, ProjectionReceiptIdentity};
use maestro_kernel::{
    artifact::Digest,
    document::Collection,
    facts::ProjectionReceipt,
    generation::NewGeneration,
    job::{JobState, Lease, LeaseTiming, NewJob},
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use rusqlite::{Connection, params};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

pub(super) struct Fixture {
    pub(super) path: PathBuf,
    pub(super) database: Database,
    pub(super) scopes: ScopeSet,
    pub(super) receipt: ProjectionReceipt,
    /// Drops after the database; Windows cannot remove an open SQLite file.
    _scratch: Scratch,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let path = scratch_directory().unwrap();
        let database = Database::open_in(&path).unwrap();
        database
            .record_collection(&Collection {
                id: "cleanup".into(),
                title: "Synthetic cleanup".into(),
                visibility: "public".into(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        let scope: Scope = "workspace/default/collection/cleanup".parse().unwrap();
        database
            .grant("cleaner", &scope, Right::Read, "test")
            .unwrap();
        let scopes = database.visible("cleaner").unwrap();
        let connection = Connection::open(path.join("kernel.sqlite3")).unwrap();
        connection
            .execute(
                "INSERT INTO chunk_sets \
             (id, collection_id, chunk_profile, counter_contract_id, state) \
             VALUES ('chunks', 'cleanup', 'test', 'test', 'building')",
                [],
            )
            .unwrap();
        let generation = database
            .create_generation(&NewGeneration {
                collection_id: "cleanup".into(),
                chunk_set_id: "chunks".into(),
                embedding_profile: "test".into(),
                sparse_profile: "test".into(),
            })
            .unwrap()
            .id;
        let set = seed_attachment(&database, &connection, &scope, generation);
        let file_name = super::super::content::basename(
            &super::super::ProjectionScope {
                collection_id: "cleanup".into(),
                generation_id: generation,
            },
            &set,
        )
        .unwrap();
        let resolution = database
            .record_resolution(
                &scopes,
                "cleaner",
                &ResolutionInput {
                    resolver_version: EXACT_RESOLVER_VERSION.into(),
                    sets: vec![set.clone()],
                    previous: None,
                    decisions: vec![],
                },
                &|_| Ok(()),
            )
            .unwrap()
            .id;
        let mut receipt = ProjectionReceipt {
            identity: ProjectionReceiptIdentity {
                build_id: generation,
                collection_id: "cleanup".into(),
                generation_id: generation,
                claim_set_id: set,
                file_name,
                schema_version: "maestro-typed-edges/3".into(),
                knowledge_edge_count: 0,
                catalog_dependency_edge_count: 0,
                entity_fact_count: 0,
                content_digest: super::super::content::digest(&[], &[]).unwrap(),
            },
            resolution_id: resolution,
            resolver_version: EXACT_RESOLVER_VERSION.into(),
            settings_identity: Digest::of(b"settings"),
            frozen_lock: Digest::of(b"frozen-lock"),
        };
        record_ready(&database, &scopes, &mut receipt);
        let graph = path.join("graph");
        let root = OwnedRoot::open(&graph, true).unwrap();
        root.ensure_control(ControlFile::Access).unwrap();
        root.ensure_control(ControlFile::Writer).unwrap();
        // The engine suite uses the same authority and synchronization with a real native file.
        #[cfg(all(feature = "engine", unix))]
        super::super::engine::cleanup_tests::install(
            &graph,
            &super::super::ProjectionScope {
                collection_id: receipt.identity.collection_id.clone(),
                generation_id: generation,
            },
            &receipt.identity.file_name,
            super::super::binding::receipt_pins(&receipt),
        );
        #[cfg(not(all(feature = "engine", unix)))]
        fs::write(graph.join(&receipt.identity.file_name), b"disposable").unwrap();
        fs::write(graph.join("orphan.lbdb"), b"preserve orphan exactly").unwrap();
        let scratch = Scratch(path.clone());
        Self {
            path,
            database,
            scopes,
            receipt,
            _scratch: scratch,
        }
    }

    /// Recreate a real pre-pin receipt, then let the current kernel migrate it.
    #[cfg(unix)]
    pub(super) fn migrate_legacy(&mut self) {
        let connection = Connection::open(self.path.join("kernel.sqlite3")).unwrap();
        connection
            .execute_batch(
                "DROP TABLE graph_projection_active;
            DROP TABLE graph_projection_receipts;
            DROP TABLE graph_projection_builds;
            DELETE FROM migrations WHERE name IN
                ('0030_graph_input_pins', '0031_graph_projection_builds');",
            )
            .unwrap();
        connection
            .execute_batch(include_str!(
                "../../../../../maestro-kernel/migrations/0019_graph_projection.sql"
            ))
            .unwrap();
        connection
            .execute(
                "INSERT INTO graph_projection_receipts
            (generation_id, collection_id, claim_set_id, file_name, schema_version,
             knowledge_edge_count, catalog_dependency_edge_count, entity_fact_count, content_digest)
            VALUES (?1, ?2, ?3, ?4, 'maestro-typed-edges/1', 0, 0, 0, ?5)",
                params![
                    self.receipt.identity.generation_id,
                    self.receipt.identity.collection_id,
                    self.receipt.identity.claim_set_id.as_str(),
                    self.receipt.identity.file_name,
                    self.receipt.identity.content_digest.as_str()
                ],
            )
            .unwrap();
        drop(connection);
        self.database = Database::open_in(&self.path).unwrap();
    }

    pub(super) fn retire(&self) {
        self.database
            .publish_generation(self.receipt.identity.generation_id)
            .unwrap();
        self.database
            .retire_generation(self.receipt.identity.generation_id)
            .unwrap();
    }

    pub(super) fn lease(&self) -> Lease {
        let inputs = json!({"generation": self.receipt.identity.generation_id});
        let scope = "workspace/default/collection/cleanup".parse().unwrap();
        let resource = format!("graph-cleanup:{}", self.receipt.identity.generation_id);
        let job = self
            .database
            .submit_job(
                &NewJob {
                    kind: "knowledge.graph.cleanup",
                    inputs: &inputs,
                    scope: &scope,
                    resource: Some(&resource),
                },
                SystemTime::now(),
            )
            .unwrap();
        self.database
            .take_job(job.id, "cleaner", SystemTime::now(), timing().term)
            .unwrap()
    }
}

/// Directory cleanup is last, after every fixture-owned database handle closes.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

pub(super) fn timing() -> LeaseTiming {
    LeaseTiming {
        now: SystemTime::now(),
        term: Duration::from_secs(30),
    }
}

/// Persist immutable readiness with the real scoped projection-job fence.
fn record_ready(database: &Database, scopes: &ScopeSet, receipt: &mut ProjectionReceipt) {
    let scope = super::super::ProjectionScope {
        collection_id: receipt.identity.collection_id.clone(),
        generation_id: receipt.identity.generation_id,
    };
    let request = ProjectionBuildRequest {
        collection_id: receipt.identity.collection_id.clone(),
        generation_id: receipt.identity.generation_id,
        claim_set_id: receipt.identity.claim_set_id.clone(),
        resolution_id: receipt.resolution_id.clone(),
        resolver_version: receipt.resolver_version.clone(),
        settings_identity: receipt.settings_identity.clone(),
        frozen_lock: receipt.frozen_lock.clone(),
        expected_active_build_id: None,
    };
    let (build_id, lease) =
        super::reserved::reserve_request((database, scopes), &request, SystemTime::now());
    receipt.identity.build_id = build_id;
    receipt.identity.file_name =
        super::super::content::build_basename(&scope, &receipt.identity.claim_set_id, build_id)
            .unwrap();
    database
        .record_projection_ready(scopes, receipt, &lease, SystemTime::now())
        .unwrap();
    database
        .complete_job(&lease, JobState::Succeeded, &json!({}))
        .unwrap();
}

/// Seed one minimal frozen authority set and its verified generation attachment.
fn seed_attachment(
    database: &Database,
    connection: &Connection,
    scope: &Scope,
    generation: i64,
) -> Digest {
    let build = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.build",
                inputs: &json!({}),
                scope,
                resource: None,
            },
            SystemTime::now(),
        )
        .unwrap();
    let set = Digest::of(b"synthetic claim set");
    // A minimal synthetic frozen set; no native engine or source bytes are needed for removal.
    connection
        .execute(
            "INSERT INTO claim_sets (id, collection_id, member_count) \
         VALUES (?1, 'cleanup', 1)",
            [set.as_str()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO graph_builds \
         (job_id, collection_id, extractor, profile_digest, sources_json, \
          batch_count, max_claims, max_rejections, claim_set_id) \
         VALUES (?1, 'cleanup', 'test', ?2, '[\"synthetic\"]', 1, 1, 0, ?2)",
            params![build.id.to_string(), set.as_str()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO graph_attachments (generation_id, job_id, claim_set_id) \
         VALUES (?1, ?2, ?3)",
            params![generation, build.id.to_string(), set.as_str()],
        )
        .unwrap();
    database.verify_generation(generation, 0).unwrap();
    set
}
