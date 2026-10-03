//! Real rule-build, attachment and readiness fixtures for the cleanup command.
use super::{
    graph_build::{exited, pilot, text},
    support::Home,
};
use maestro_canonicalization::{ControlFile, OwnedRoot};
use maestro_kernel::{
    artifact::Digest,
    facts::ProjectionReceipt,
    generation::NewGeneration,
    job::{JobState, NewJob},
    scope::LOCAL,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    fs,
    time::{Duration, SystemTime},
};

/// One real rule build, generation attachment and immutable readiness receipt.
pub(super) fn retired(home: &Home) -> (i64, String) {
    let rule = pilot(home);
    let built = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "build",
        "--collection",
        "synthetic-graph",
        "--rule",
        text(&rule),
    ]);
    exited(&built, 0);
    let build: ulid::Ulid = built.json()["job"].as_str().unwrap().parse().unwrap();
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let generation = generation(home);
    let scope = "workspace/default/collection/synthetic-graph"
        .parse()
        .unwrap();
    let inputs = json!({"build": build.to_string(), "generation": generation});
    let attach = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.attach",
                inputs: &inputs,
                scope: &scope,
                resource: None,
            },
            SystemTime::now(),
        )
        .unwrap();
    let lease = database
        .take_job(
            attach.id,
            "attacher",
            SystemTime::now(),
            Duration::from_secs(30),
        )
        .unwrap();
    let attachment = database
        .attach_claim_set(&scopes, generation, build, &lease)
        .unwrap();
    database
        .complete_job(&lease, JobState::Succeeded, &json!({}))
        .unwrap();
    database.verify_generation(generation, 0).unwrap();
    let name = format!(
        "g{}.lbdb",
        Digest::of(generation.to_string().as_bytes()).as_str()
    );
    let inputs = json!({"generation": generation});
    let project = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.project",
                inputs: &inputs,
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
    database
        .record_projection_ready(
            &scopes,
            &ProjectionReceipt {
                collection_id: "synthetic-graph".into(),
                generation_id: generation,
                claim_set_id: attachment.claim_set_id,
                file_name: name.clone(),
                schema_version: "maestro-typed-edges/1".into(),
                knowledge_edge_count: 0,
                catalog_dependency_edge_count: 0,
                entity_fact_count: 4,
                content_digest: Digest::of(b"disposable"),
            },
            &lease,
        )
        .unwrap();
    database
        .complete_job(&lease, JobState::Succeeded, &json!({}))
        .unwrap();
    database.publish_generation(generation).unwrap();
    database.retire_generation(generation).unwrap();
    guards(home);
    fs::write(home.data().join("graph").join(&name), b"disposable").unwrap();
    (generation, name)
}

/// A fresh unpublished synthetic generation.
fn generation(home: &Home) -> i64 {
    let database = home.database();
    Connection::open(home.data().join("kernel.sqlite3"))
        .unwrap()
        .execute(
            "INSERT INTO chunk_sets \
            (id, collection_id, chunk_profile, counter_contract_id, state) \
            VALUES ('cleanup-chunks', 'synthetic-graph', 'test', 'test', 'building')",
            [],
        )
        .unwrap();
    database
        .create_generation(&NewGeneration {
            collection_id: "synthetic-graph".into(),
            chunk_set_id: "cleanup-chunks".into(),
            embedding_profile: "test".into(),
            sparse_profile: "test".into(),
        })
        .unwrap()
        .id
}

pub(super) fn guards(home: &Home) {
    let root = OwnedRoot::open(&home.data().join("graph"), true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    root.ensure_control(ControlFile::Writer).unwrap();
}

pub(super) fn document(
    generation: i64,
    name: &str,
    action: &str,
    reason: &str,
    present: bool,
) -> Value {
    json!({
        "schema": "maestro-cli/knowledge-graph-cleanup/1",
        "action": action, "reason": reason, "generation": generation,
        "collection": "synthetic-graph", "file_name": name, "present": present
    })
}
