//! Projection readiness receipt and lease validation tests.

use super::support::{Scratch, build_job, execute, granted, label, plan, timing};
use crate::facts::ResolutionInput;
use crate::facts::{EXACT_RESOLVER_VERSION, ProjectionReceiptIdentity};
use crate::{
    artifact::Digest,
    document::Collection,
    facts::{
        Batch, Error, PROJECTION_REBUILD_REPAIR, ProjectionReceipt, Rejection,
        projection_inventory_in,
    },
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
    granted(&database, "projection", "workspace/default");
    let resolution = database
        .record_resolution(
            &all,
            "projection",
            &ResolutionInput {
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                sets: vec![attachment.claim_set_id.clone()],
                previous: None,
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap()
        .id;
    let receipt = ProjectionReceipt {
        identity: ProjectionReceiptIdentity {
            collection_id: "graph".to_owned(),
            generation_id: generation,
            claim_set_id: attachment.claim_set_id,
            file_name: format!("projection-{generation}.db"),
            schema_version: "maestro-typed-edges/2".to_owned(),
            knowledge_edge_count: 0,
            catalog_dependency_edge_count: 0,
            entity_fact_count: 1,
            content_digest: Digest::of(b"verified projection"),
        },
        resolution_id: resolution,
        resolver_version: EXACT_RESOLVER_VERSION.into(),
        settings_identity: Digest::of(b"settings"),
        frozen_lock: Digest::of(b"frozen-lock"),
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
    let lease = projection_lease(&database, receipt.identity.generation_id);
    let claim_set_id = receipt.identity.claim_set_id.clone();
    receipt.identity.entity_fact_count = 0;
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
    receipt.identity.entity_fact_count = 1;
    receipt.identity.knowledge_edge_count = 1;
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
    receipt.identity.knowledge_edge_count = 0;
    receipt.identity.claim_set_id = Digest::of(b"wrong claim set");
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
    receipt.identity.claim_set_id = claim_set_id;
    assert_eq!(
        database
            .projection_ready(&all, receipt.identity.generation_id)
            .unwrap(),
        None
    );

    receipt.identity.entity_fact_count = 1;
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    assert_eq!(
        database
            .projection_ready(&all, receipt.identity.generation_id)
            .unwrap(),
        Some(receipt)
    );
}

#[test]
fn a_projection_receipt_is_once_only_and_scoped_to_its_generation() {
    let (scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
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
            .projection_ready(&denied, receipt.identity.generation_id)
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
    let lease = projection_lease(&database, receipt.identity.generation_id);
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    database
        .publish_generation(receipt.identity.generation_id)
        .unwrap();
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
    assert_eq!(inventory[1].collection_id, receipt.identity.collection_id);
    assert_eq!(inventory[1].generation_id, receipt.identity.generation_id);
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
    let lease = projection_lease(&database, receipt.identity.generation_id);
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    database
        .publish_generation(receipt.identity.generation_id)
        .unwrap();
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
            [receipt.identity.generation_id],
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
    let lease = projection_lease(&database, receipt.identity.generation_id);
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    database
        .publish_generation(receipt.identity.generation_id)
        .unwrap();
    database
        .retire_generation(receipt.identity.generation_id)
        .unwrap();
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
    let lease = projection_lease(&database, receipt.identity.generation_id);
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
fn projection_readiness_rejects_an_expired_projection_lease() {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    database
        .take_job(lease.job, "takeover", timing(100).now, timing(100).term)
        .unwrap();
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Job(job::Error::Lost { .. }))
    ));
    assert_eq!(
        database
            .projection_ready(&all, receipt.identity.generation_id)
            .unwrap(),
        None
    );
}

#[test]
fn a_building_generation_cannot_record_projection_readiness() {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    execute(
        &database,
        &format!(
            "UPDATE generations SET state = 'building' WHERE id = {}",
            receipt.identity.generation_id
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
    let lease = projection_lease(&database, receipt.identity.generation_id);
    receipt.identity.generation_id = 0;
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
}

#[test]
fn projection_readiness_rejects_a_path_instead_of_a_owned_filename() {
    let (_scratch, database, all, mut receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    for name in [
        "../outside.db",
        "safe/outside.db",
        ".leading.db",
        "",
        "space name.db",
    ] {
        receipt.identity.file_name = name.to_owned();
        let error = database
            .record_projection_ready(&all, &receipt, &lease, timing(5).now)
            .unwrap_err();
        let Error::Conflict(detail) = error else {
            panic!("{name}: {error}");
        };
        assert_eq!(
            detail,
            format!("invalid projection receipt identity; {PROJECTION_REBUILD_REPAIR}"),
            "{name}"
        );
    }
    assert_eq!(
        database
            .projection_ready(&all, receipt.identity.generation_id)
            .unwrap(),
        None
    );
}

#[test]
fn projection_health_config_is_evaluated_without_persisting_grants() {
    use crate::{
        facts::{InventoryState, projection_inventory_with_config},
        scope::{Config, LOCAL},
    };
    let (scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    database
        .publish_generation(receipt.identity.generation_id)
        .unwrap();
    let config: Config = "[access]\nread = ['workspace/default/collection/graph']"
        .parse()
        .unwrap();
    assert!(database.visible(LOCAL).unwrap().is_empty());
    let InventoryState::Ready(rows) =
        projection_inventory_with_config(&scratch.0, &config).unwrap()
    else {
        panic!("current authority expected");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].receipt.as_ref(), Some(&receipt));
    assert!(
        database.visible(LOCAL).unwrap().is_empty(),
        "health must not apply grants"
    );
    database.apply_config(&config).unwrap();
    assert_eq!(
        projection_inventory_with_config(&scratch.0, &Config::default()).unwrap(),
        InventoryState::Ready(Vec::new()),
        "current config revocation overrides stored grants"
    );
}
