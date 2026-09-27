//! Independent exact-identifier leg failures and empty results.

use super::{
    backends::fake,
    identifier_route::{
        PublishedCommand, identifier_search, identifier_search_until, publish_command,
    },
    support::cleanup,
};
use maestro_kernel::{evidence::RouteStatus, generation::NewGeneration};
use std::time::Duration;
use tokio::time::Instant;

#[tokio::test]
async fn both_identifier_legs_succeed_empty_for_an_unknown_literal() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let outcome = identifier_search(&fixture, "ERR-999").await;
    assert_eq!(outcome.status, RouteStatus::Ok);
    assert!(outcome.hits.is_empty());
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn an_identifiers_one_generation_requires_a_new_generation() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let old = fixture
        .kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: fixture.generation.collection_id.clone(),
            chunk_set_id: fixture.generation.chunk_set_id.clone(),
            embedding_profile: fixture.generation.embedding_profile.clone(),
            sparse_profile: fixture.generation.sparse_profile.clone(),
        })
        .unwrap();
    fixture
        .kernel
        .database
        .begin_generation_search(&fixture.kernel.scopes, old.id, "identifiers/1")
        .unwrap();
    fixture
        .kernel
        .database
        .complete_generation_search(&fixture.kernel.scopes, old.id)
        .unwrap();
    fixture
        .kernel
        .database
        .verify_generation(old.id, 0)
        .unwrap();
    fixture.kernel.database.publish_generation(old.id).unwrap();
    let old = fixture
        .kernel
        .database
        .generation(&fixture.kernel.scopes, old.id)
        .unwrap()
        .unwrap();
    let published = fixture.generation.clone();
    let old_fixture = PublishedCommand {
        kernel: fixture.kernel,
        qdrant: fixture.qdrant,
        generation: old,
    };

    let outcome = identifier_search(&old_fixture, "ERR-042").await;
    assert!(matches!(
        &outcome.status,
        RouteStatus::Unavailable(reason) if reason.contains("publish a new generation")
    ));
    cleanup(&backend, &[&published]).await;
}

#[tokio::test]
async fn a_failed_kernel_leg_keeps_the_successful_payload_hit() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let connection = rusqlite::Connection::open(fixture.kernel.database_path()).unwrap();
    connection
        .execute("DROP TABLE chunk_search_identifiers", [])
        .unwrap();
    drop(connection);

    let outcome = identifier_search(&fixture, "ERR-042").await;
    assert!(matches!(
        &outcome.status,
        RouteStatus::Unavailable(reason) if reason.starts_with("kernel:")
    ));
    assert!(
        outcome
            .hits
            .iter()
            .any(|hit| hit.chunk_id == "chunk-0-lead")
    );
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn a_payload_timeout_keeps_the_completed_kernel_hit() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let gate = backend.fake.as_ref().unwrap().gate_next_scroll();

    let outcome = identifier_search_until(
        &fixture,
        "ERR-042",
        20,
        None,
        Instant::now() + Duration::from_millis(200),
    )
    .await;
    assert!(matches!(
        &outcome.status,
        RouteStatus::Unavailable(reason) if reason.starts_with("payload: route deadline elapsed")
    ));
    assert!(
        outcome
            .hits
            .iter()
            .any(|hit| hit.chunk_id == "chunk-0-lead")
    );
    gate.notify_one();
    cleanup(&backend, &[&fixture.generation]).await;
}
