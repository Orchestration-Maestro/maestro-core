//! Synthetic cleanup authority under the same private CLI apply boundary.
use super::runner_tests::support::{Fixture, fixture};
use maestro_filesystem::{ControlFile, OwnedRoot};
use maestro_kernel::facts::EXACT_RESOLVER_VERSION;
use maestro_kernel::facts::ProjectionReceiptIdentity;
use maestro_kernel::facts::ResolutionInput;
use maestro_kernel::scope::{LOCAL, ScopeSet};
use maestro_kernel::store::Database;
use maestro_kernel::{
    artifact::Digest,
    facts::ProjectionReceipt,
    generation::NewGeneration,
    job::{JobState, NewJob},
};
use rusqlite::{Connection, params};
use serde_json::json;
use std::{
    fs,
    time::{Duration, SystemTime},
};

pub(in crate::cli::graph) fn cleanup_fixture() -> (Fixture, i64, String) {
    let mut fixture = fixture(&[]);
    fixture.kernel.config_dir = fixture.root.clone();
    fs::write(
        fixture.root.join("config.toml"),
        "[access]\nread = [\"workspace/default/collection/graph-test\"]\n",
    )
    .unwrap();
    fixture.kernel.refresh_scopes().unwrap();
    let database = &fixture.kernel.database;
    let connection = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    connection
        .execute(
            "INSERT INTO chunk_sets \
             (id, collection_id, chunk_profile, counter_contract_id, state) \
             VALUES ('cleanup-chunks', 'graph-test', 'test', 'test', 'building')",
            [],
        )
        .unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "graph-test".into(),
            chunk_set_id: "cleanup-chunks".into(),
            embedding_profile: "test".into(),
            sparse_profile: "test".into(),
        })
        .unwrap()
        .id;
    let scope = "workspace/default/collection/graph-test".parse().unwrap();
    let build = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.build",
                inputs: &json!({}),
                scope: &scope,
                resource: None,
            },
            SystemTime::now(),
        )
        .unwrap();
    let set = Digest::of(b"synthetic cleanup set");
    seed_attachment(&connection, generation, &build.id.to_string(), &set);
    database.verify_generation(generation, 0).unwrap();
    let name = format!("g{}.lbdb", Digest::of(b"synthetic cleanup file").as_str());
    let project = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.project",
                inputs: &json!({"generation": generation}),
                scope: &scope,
                resource: None,
            },
            SystemTime::now(),
        )
        .unwrap();
    let lease = database
        .take_job(
            project.id,
            "projector",
            SystemTime::now(),
            Duration::from_secs(30),
        )
        .unwrap();
    let resolution = frozen_resolution(database, &fixture.kernel.scopes, &set);
    database
        .record_projection_ready(
            &fixture.kernel.scopes,
            &ProjectionReceipt {
                identity: ProjectionReceiptIdentity {
                    build_id: generation,
                    collection_id: "graph-test".into(),
                    generation_id: generation,
                    claim_set_id: set,
                    file_name: name.clone(),
                    schema_version: "maestro-typed-edges/2".into(),
                    knowledge_edge_count: 0,
                    catalog_dependency_edge_count: 0,
                    entity_fact_count: 0,
                    content_digest: Digest::of(b"disposable"),
                },
                resolution_id: resolution,
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                settings_identity: Digest::of(b"settings"),
                frozen_lock: Digest::of(b"frozen-lock"),
            },
            &lease,
            SystemTime::now(),
        )
        .unwrap();
    database
        .complete_job(&lease, JobState::Succeeded, &json!({}))
        .unwrap();
    database.publish_generation(generation).unwrap();
    database.retire_generation(generation).unwrap();
    let root = OwnedRoot::open(&fixture.root.join("graph"), true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    fs::write(fixture.root.join("graph").join(&name), b"disposable").unwrap();
    (fixture, generation, name)
}

/// Minimal frozen synthetic authority; no engine or privileged fixture is involved.
fn seed_attachment(connection: &Connection, generation: i64, build: &str, set: &Digest) {
    connection
        .execute(
            "INSERT INTO claim_sets (id, collection_id, member_count) \
             VALUES (?1, 'graph-test', 1)",
            [set.as_str()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO graph_builds \
             (job_id, collection_id, extractor, profile_digest, sources_json, \
              batch_count, max_claims, max_rejections, claim_set_id) \
             VALUES (?1, 'graph-test', 'test', ?2, '[\"synthetic\"]', 1, 1, 0, ?2)",
            params![build, set.as_str()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO graph_attachments (generation_id, job_id, claim_set_id) \
             VALUES (?1, ?2, ?3)",
            params![generation, build, set.as_str()],
        )
        .unwrap();
}

/// Freeze the explicit source set used by this projection fixture.
fn frozen_resolution(database: &Database, scopes: &ScopeSet, set: &Digest) -> Digest {
    database
        .record_resolution(
            scopes,
            LOCAL,
            &ResolutionInput {
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                sets: vec![set.clone()],
                previous: None,
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap()
        .id
}
