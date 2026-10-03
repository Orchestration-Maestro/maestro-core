//! Projection readiness receipt and lease validation tests.

use super::support::{Scratch, build_job, execute, granted, label, plan, timing};
use crate::{
    artifact::Digest,
    document::Collection,
    facts::{Batch, Error, ProjectionReceipt, Rejection, projection_inventory_in},
    generation::NewGeneration,
    job::{self, JobState},
    scope::{Right, Scope, ScopeSet},
    store::{
        Database,
        database::{HealthOpen, open_health_in},
    },
};
use serde_json::json;
use std::{collections::BTreeMap, thread};

pub(super) fn attached() -> (Scratch, Database, ScopeSet, ProjectionReceipt) {
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
        catalog_dependency_edge_count: 0,
        entity_fact_count: 1,
        content_digest: Digest::of(b"verified projection"),
    };
    (scratch, database, all, receipt)
}

pub(super) fn projection_lease(database: &Database, generation: i64) -> job::Lease {
    let scope: Scope = "workspace/default/collection/graph".parse().unwrap();
    let inputs = json!({"generation": generation});
    let job = database
        .submit_job(
            &job::NewJob {
                kind: "knowledge.graph.project",
                inputs: &inputs,
                scope: &scope,
                resource: None,
            },
            timing(4).now,
        )
        .unwrap();
    job.lease.unwrap_or_else(|| {
        database
            .take_job(job.id, "projector", timing(4).now, timing(4).term)
            .unwrap()
    })
}

#[test]
fn readiness_is_kernel_controlled_and_matches_the_attached_claim_set() {
    let (_scratch, database, all, mut receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    let claim_set_id = receipt.claim_set_id.clone();
    receipt.entity_fact_count = 0;
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
    receipt.entity_fact_count = 1;
    receipt.knowledge_edge_count = 1;
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
    receipt.knowledge_edge_count = 0;
    receipt.claim_set_id = Digest::of(b"wrong claim set");
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
    receipt.claim_set_id = claim_set_id;
    assert_eq!(
        database
            .projection_ready(&all, receipt.generation_id)
            .unwrap(),
        None
    );

    receipt.entity_fact_count = 1;
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
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
    let lease = projection_lease(&database, receipt.generation_id);
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
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
fn health_inventory_is_scoped_ordered_and_decodes_the_exact_receipt() {
    let (scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    database.publish_generation(receipt.generation_id).unwrap();
    database
        .record_collection(&Collection {
            id: "alpha".to_owned(),
            title: "Alpha collection".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    execute(
        &database,
        "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state)
         VALUES ('alpha-set', 'alpha', 'structural/1', 'native', 'building')",
    )
    .unwrap();
    execute(
        &database,
        "UPDATE chunk_sets SET state = 'complete', manifest_digest =
         '4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945'
         WHERE id = 'alpha-set'",
    )
    .unwrap();
    let alpha = database
        .create_generation(&NewGeneration {
            collection_id: "alpha".to_owned(),
            chunk_set_id: "alpha-set".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap()
        .id;
    database.verify_generation(alpha, 0).unwrap();
    database.publish_generation(alpha).unwrap();

    let health = match open_health_in(&scratch.0).unwrap() {
        HealthOpen::Ready(health) => health,
        other => panic!("expected current kernel, got {other:?}"),
    };
    let inventory = health.current_projection_inventory(&all).unwrap();
    assert_eq!(inventory.len(), 2);
    assert_eq!(inventory[0].collection_id, "alpha");
    assert_eq!(inventory[0].generation_id, alpha);
    assert_eq!(inventory[0].receipt, None);
    assert_eq!(inventory[1].collection_id, receipt.collection_id);
    assert_eq!(inventory[1].generation_id, receipt.generation_id);
    assert_eq!(inventory[1].receipt.as_ref(), Some(&receipt));

    let denied = granted(
        &database,
        "health-denied",
        "workspace/default/collection/other",
    );
    assert!(
        health
            .current_projection_inventory(&denied)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn projection_inventory_preserves_typed_receipt_decode_errors() {
    let (scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    database.publish_generation(receipt.generation_id).unwrap();
    let scope: Scope = "workspace/default/collection/graph".parse().unwrap();
    database
        .grant("inventory-reader", &scope, Right::Read, "test")
        .unwrap();
    drop(database);

    let outside = scratch.outside();
    outside
        .execute("DROP TRIGGER graph_projection_receipts_never_changed", [])
        .unwrap();
    outside
        .execute(
            "UPDATE graph_projection_receipts SET content_digest =
             'zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz'
             WHERE generation_id = ?1",
            [receipt.generation_id],
        )
        .unwrap();
    assert!(matches!(
        projection_inventory_in(&scratch.0, "inventory-reader"),
        Err(Error::Conflict(_))
    ));
}

#[test]
fn health_inventory_omits_retired_receipts_and_unpublished_verified_generations() {
    let (scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    database.publish_generation(receipt.generation_id).unwrap();
    database.retire_generation(receipt.generation_id).unwrap();
    execute(
        &database,
        "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state)
         VALUES ('future-set', 'graph', 'structural/1', 'native', 'building')",
    )
    .unwrap();
    execute(
        &database,
        "UPDATE chunk_sets SET state = 'complete', manifest_digest =
         '4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945'
         WHERE id = 'future-set'",
    )
    .unwrap();
    let verified = database
        .create_generation(&NewGeneration {
            collection_id: "graph".to_owned(),
            chunk_set_id: "future-set".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap()
        .id;
    database.verify_generation(verified, 0).unwrap();

    let HealthOpen::Ready(health) = open_health_in(&scratch.0).unwrap() else {
        panic!("expected current kernel");
    };
    assert!(
        health
            .current_projection_inventory(&all)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn concurrent_readiness_recorders_have_one_winner() {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    let (first, second) = thread::scope(|scope| {
        let first =
            scope.spawn(|| database.record_projection_ready(&all, &receipt, &lease, timing(5).now));
        let second =
            scope.spawn(|| database.record_projection_ready(&all, &receipt, &lease, timing(5).now));
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_ne!(first.is_ok(), second.is_ok());
    assert!(matches!(
        first.err().or_else(|| second.err()),
        Some(Error::Conflict(_))
    ));
}

#[test]
fn raw_receipt_insert_must_match_the_generation_attachment() {
    let (scratch, _database, _all, receipt) = attached();
    let outside = scratch.outside();
    assert!(
        outside
            .execute(
                "INSERT INTO graph_projection_receipts
         (generation_id, collection_id, claim_set_id, file_name, schema_version,
          knowledge_edge_count, catalog_dependency_edge_count, entity_fact_count, content_digest)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    receipt.generation_id,
                    receipt.collection_id,
                    receipt.claim_set_id.as_str(),
                    receipt.file_name,
                    receipt.schema_version,
                    1_i64,
                    0_i64,
                    1_i64,
                    receipt.content_digest.as_str()
                ],
            )
            .is_err()
    );
}

#[test]
fn projection_readiness_rejects_an_expired_projection_lease() {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    database
        .take_job(lease.job, "takeover", timing(100).now, timing(100).term)
        .unwrap();
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Job(job::Error::Lost { .. }))
    ));
    assert_eq!(
        database
            .projection_ready(&all, receipt.generation_id)
            .unwrap(),
        None
    );
}

#[test]
fn a_building_generation_cannot_record_projection_readiness() {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    execute(
        &database,
        &format!(
            "UPDATE generations SET state = 'building' WHERE id = {}",
            receipt.generation_id
        ),
    )
    .unwrap();
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Unauthorized)
    ));
}

#[test]
fn projection_readiness_rejects_a_nonpositive_generation() {
    let (_scratch, database, all, mut receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    receipt.generation_id = 0;
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
}

#[test]
fn projection_readiness_rejects_a_path_instead_of_a_owned_filename() {
    let (_scratch, database, all, mut receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    receipt.file_name = "../outside.db".to_owned();
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
    assert_eq!(
        database
            .projection_ready(&all, receipt.generation_id)
            .unwrap(),
        None
    );
}
