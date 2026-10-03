//! Valid neighbours for cleanup state, authority and leaf-boundary refusals.
use super::super::cleanup::{Cleanup, CleanupError};
use super::cleanup_support::{Fixture, timing};
use maestro_kernel::{
    generation::NewGeneration,
    job::{JobState, NewJob},
};
use serde_json::json;
use std::{collections::BTreeMap, fs, time::SystemTime};

#[test]
fn cleanup_building_and_receiptless_failed_preserve_orphans() {
    let fixture = Fixture::new();
    fixture.retire();
    let generation = fixture
        .database
        .create_generation(&NewGeneration {
            collection_id: "cleanup".into(),
            chunk_set_id: "chunks".into(),
            embedding_profile: "test".into(),
            sparse_profile: "test".into(),
        })
        .unwrap()
        .id;
    let graph = fixture.path.join("graph");
    assert_eq!(
        Cleanup::prepare(&fixture.database, "cleaner", &graph, generation, true).unwrap_err(),
        CleanupError::Ineligible
    );
    fixture.database.fail_generation(generation).unwrap();
    assert_eq!(
        Cleanup::prepare(&fixture.database, "cleaner", &graph, generation, false).unwrap_err(),
        CleanupError::ReceiptMissing
    );
    assert!(
        Cleanup::prepare(
            &fixture.database,
            "cleaner",
            &graph,
            fixture.receipt.generation_id,
            true
        )
        .is_ok()
    );
    assert_eq!(
        fs::read(graph.join("orphan.lbdb")).unwrap(),
        b"preserve orphan exactly"
    );
}

#[test]
fn cleanup_lease_requires_exact_kind_scope_resource_inputs_number_and_holder() {
    let fixture = Fixture::new();
    fixture.retire();
    let cleanup = Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &fixture.path.join("graph"),
        fixture.receipt.generation_id,
        true,
    )
    .unwrap();
    let resource = format!("graph-cleanup:{}", fixture.receipt.generation_id);
    let inputs = json!({"generation": fixture.receipt.generation_id});
    let wrong = json!({"generation": fixture.receipt.generation_id + 1});
    for (kind, scope, resource, inputs) in [
        (
            "knowledge.graph.cleanup.wrong",
            "workspace/default/collection/cleanup",
            Some(resource.as_str()),
            &inputs,
        ),
        (
            "knowledge.graph.cleanup",
            "workspace/default",
            Some(resource.as_str()),
            &inputs,
        ),
        (
            "knowledge.graph.cleanup",
            "workspace/default/collection/cleanup",
            Some("wrong-resource"),
            &inputs,
        ),
        (
            "knowledge.graph.cleanup",
            "workspace/default/collection/cleanup",
            Some(resource.as_str()),
            &wrong,
        ),
    ] {
        let scope = scope.parse().unwrap();
        let job = fixture
            .database
            .submit_job(
                &NewJob {
                    kind,
                    inputs,
                    scope: &scope,
                    resource,
                },
                SystemTime::now(),
            )
            .unwrap();
        let mut lease = fixture
            .database
            .take_job(job.id, "cleaner", SystemTime::now(), timing().term)
            .unwrap();
        assert_eq!(
            cleanup
                .apply(&fixture.database, "cleaner", &mut lease, timing())
                .unwrap_err(),
            CleanupError::LeaseInvalid
        );
        fixture
            .database
            .complete_job(&lease, JobState::Failed, &json!({}))
            .unwrap();
    }
    let mut valid = fixture.lease();
    let mut stale = valid.clone();
    stale.number += 1;
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut stale, timing())
            .unwrap_err(),
        CleanupError::LeaseInvalid
    );
    stale = valid.clone();
    stale.holder = "not-holder".into();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut stale, timing())
            .unwrap_err(),
        CleanupError::LeaseInvalid
    );
    #[cfg(unix)]
    assert!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut valid, timing())
            .is_ok()
    );
    #[cfg(windows)]
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut valid, timing())
            .unwrap_err(),
        CleanupError::Unsupported
    );
}

#[cfg(unix)]
#[test]
fn cleanup_symlink_hard_link_and_root_relocation_refuse_with_valid_neighbour() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fixture.retire();
    let graph = fixture.path.join("graph");
    let path = graph.join(&fixture.receipt.file_name);
    let sentinel = fixture.path.join("sentinel");
    fs::write(&sentinel, b"outside sentinel").unwrap();
    fs::remove_file(&path).unwrap();
    symlink(&sentinel, &path).unwrap();
    assert_eq!(
        Cleanup::prepare(
            &fixture.database,
            "cleaner",
            &graph,
            fixture.receipt.generation_id,
            true
        )
        .unwrap_err(),
        CleanupError::UnsafeFile
    );
    fs::remove_file(&path).unwrap();
    fs::hard_link(&sentinel, &path).unwrap();
    assert_eq!(
        Cleanup::prepare(
            &fixture.database,
            "cleaner",
            &graph,
            fixture.receipt.generation_id,
            false
        )
        .unwrap_err(),
        CleanupError::UnsafeFile
    );
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"valid neighbour").unwrap();
    let cleanup = Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &graph,
        fixture.receipt.generation_id,
        true,
    )
    .unwrap();
    fs::rename(&graph, fixture.path.join("relocated")).unwrap();
    fs::create_dir(&graph).unwrap();
    let mut lease = fixture.lease();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::UnsafeFile
    );
    drop(cleanup);
    fs::remove_dir(&graph).unwrap();
    fs::rename(fixture.path.join("relocated"), &graph).unwrap();
    let cleanup = Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &graph,
        fixture.receipt.generation_id,
        true,
    )
    .unwrap();
    assert!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .is_ok()
    );
    assert_eq!(fs::read(&sentinel).unwrap(), b"outside sentinel");
}

#[test]
fn cleanup_failed_with_receipt_has_the_same_apply_policy() {
    let fixture = Fixture::new();
    fixture
        .database
        .fail_generation(fixture.receipt.generation_id)
        .unwrap();
    let cleanup = Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &fixture.path.join("graph"),
        fixture.receipt.generation_id,
        true,
    )
    .unwrap();
    let mut lease = fixture.lease();
    #[cfg(unix)]
    assert!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .is_ok()
    );
    #[cfg(windows)]
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::Unsupported
    );
    assert_eq!(
        fixture
            .database
            .projection_ready(&fixture.scopes, fixture.receipt.generation_id)
            .unwrap(),
        Some(fixture.receipt.clone())
    );
}

#[test]
fn cleanup_corrupt_collection_and_changed_readiness_refuse_without_unlink() {
    use maestro_kernel::{artifact::Digest, document::Collection};
    use rusqlite::Connection;
    let fixture = Fixture::new();
    fixture.retire();
    let graph = fixture.path.join("graph");
    let cleanup = Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &graph,
        fixture.receipt.generation_id,
        true,
    )
    .unwrap();
    let mut lease = fixture.lease();
    // Model authority corruption, not a supported mutation: immutable update triggers stay
    // untouched in production. A malformed stored authority cannot authorize deletion.
    let connection = Connection::open(fixture.path.join("kernel.sqlite3")).unwrap();
    fixture
        .database
        .record_collection(&Collection {
            id: "corrupt".into(),
            title: "Corrupt neighbour".into(),
            visibility: "public".into(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    connection
        .execute("DROP TRIGGER graph_projection_receipts_never_changed", [])
        .unwrap();
    connection
        .execute(
            "UPDATE graph_projection_receipts SET collection_id = 'corrupt'",
            [],
        )
        .unwrap();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::UnsafeFile
    );
    connection
        .execute(
            "UPDATE graph_projection_receipts SET collection_id = 'cleanup', content_digest = ?1",
            [Digest::of(b"changed").as_str()],
        )
        .unwrap();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::UnsafeFile
    );
    assert!(graph.join(&fixture.receipt.file_name).exists());
    connection
        .execute(
            "UPDATE graph_projection_receipts SET content_digest = ?1",
            [fixture.receipt.content_digest.as_str()],
        )
        .unwrap();
    #[cfg(unix)]
    assert!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .is_ok()
    );
    #[cfg(windows)]
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::Unsupported
    );
}

#[cfg(unix)]
#[test]
fn cleanup_unavailable_authority_refuses_with_recovered_valid_neighbour() {
    let fixture = Fixture::new();
    fixture.retire();
    let graph = fixture.path.join("graph");
    let cleanup = Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &graph,
        fixture.receipt.generation_id,
        true,
    )
    .unwrap();
    let mut lease = fixture.lease();
    let kernel = fixture.path.join("kernel.sqlite3");
    let unavailable = fixture.path.join("unavailable.sqlite3");
    fs::rename(&kernel, &unavailable).unwrap();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::AuthorityUnavailable
    );
    assert!(graph.join(&fixture.receipt.file_name).exists());
    fs::rename(&unavailable, &kernel).unwrap();
    assert!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .is_ok()
    );
}
