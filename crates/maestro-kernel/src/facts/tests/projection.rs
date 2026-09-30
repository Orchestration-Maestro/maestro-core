use super::support::{Scratch, build_job, execute, granted, label, plan, timing};
use crate::{
    artifact::Digest,
    facts::{Batch, Error, ProjectionReceipt, Rejection},
    generation::NewGeneration,
    job::{self, JobState},
    scope::{Scope, ScopeSet},
    store::Database,
};
use serde_json::json;

fn attached() -> (Scratch, Database, ScopeSet, ProjectionReceipt) {
    let scratch = Scratch::new();
    let database = scratch.open();
    let all = ScopeSet::default_workspace();
    let build_plan = plan(&["rev-a"], 10, 0);
    let mut build = build_job(&database, &build_plan, "builder");
    database
        .begin_graph_build(&all, &build, &build_plan)
        .unwrap();
    database
        .record_graph_batch(
            &all,
            &mut build,
            timing(1),
            &Batch {
                ordinal: 0,
                claims: vec![label()],
                rejections: Vec::<Rejection>::new(),
            },
        )
        .unwrap();
    database
        .finish_graph_build(&all, &mut build, timing(2))
        .unwrap();
    database
        .complete_job(&build, JobState::Succeeded, &json!({}))
        .unwrap();
    execute(
        &database,
        "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state)
         VALUES ('graph-set', 'graph', 'structural/1', 'native', 'building')",
    )
    .unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "graph".to_owned(),
            chunk_set_id: "graph-set".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap()
        .id;
    let attach_inputs = json!({"build": build.job.to_string(), "generation": generation});
    let scope: Scope = "workspace/default/collection/graph".parse().unwrap();
    let attachment_job = database
        .submit_job(
            &job::NewJob {
                kind: "knowledge.graph.attach",
                inputs: &attach_inputs,
                scope: &scope,
                resource: None,
            },
            timing(3).now,
        )
        .unwrap();
    let lease = attachment_job.lease.unwrap_or_else(|| {
        database
            .take_job(attachment_job.id, "attacher", timing(3).now, timing(3).term)
            .unwrap()
    });
    let attachment = database
        .attach_claim_set(&all, generation, build.job, &lease)
        .unwrap();
    database.verify_generation(generation, 1).unwrap();
    let receipt = ProjectionReceipt {
        collection_id: "graph".to_owned(),
        generation_id: generation,
        claim_set_id: attachment.claim_set_id,
        file_name: format!("projection-{generation}.db"),
        schema_version: "maestro-typed-edges/1".to_owned(),
        knowledge_edge_count: 0,
        entity_fact_count: 1,
        content_digest: Digest::of(b"verified projection"),
    };
    (scratch, database, all, receipt)
}

#[test]
fn readiness_is_kernel_controlled_and_matches_the_attached_claim_set() {
    let (_scratch, database, all, mut receipt) = attached();
    receipt.entity_fact_count = 0;
    assert!(matches!(
        database.record_projection_ready(&all, &receipt),
        Err(Error::Conflict(_))
    ));
    assert_eq!(
        database
            .projection_ready(&all, receipt.generation_id)
            .unwrap(),
        None
    );

    receipt.entity_fact_count = 1;
    database.record_projection_ready(&all, &receipt).unwrap();
    assert_eq!(
        database
            .projection_ready(&all, receipt.generation_id)
            .unwrap(),
        Some(receipt)
    );
}

#[test]
fn a_projection_receipt_is_once_only_and_scoped_to_its_generation() {
    let (scratch, database, all, receipt) = attached();
    database.record_projection_ready(&all, &receipt).unwrap();
    assert!(matches!(
        database.record_projection_ready(&all, &receipt),
        Err(Error::Conflict(_))
    ));
    let denied = granted(
        &database,
        "projection-denied",
        "workspace/default/collection/other",
    );
    assert_eq!(
        database
            .projection_ready(&denied, receipt.generation_id)
            .unwrap(),
        None
    );
    let reader = scratch.outside();
    assert!(
        reader
            .execute(
                "UPDATE graph_projection_receipts SET file_name = 'replacement.db'",
                []
            )
            .is_err()
    );
    assert!(
        reader
            .execute("DELETE FROM graph_projection_receipts", [])
            .is_err()
    );
}

#[test]
fn projection_readiness_rejects_a_path_instead_of_a_owned_filename() {
    let (_scratch, database, all, mut receipt) = attached();
    receipt.file_name = "../outside.db".to_owned();
    assert!(matches!(
        database.record_projection_ready(&all, &receipt),
        Err(Error::Conflict(_))
    ));
    assert_eq!(
        database
            .projection_ready(&all, receipt.generation_id)
            .unwrap(),
        None
    );
}
