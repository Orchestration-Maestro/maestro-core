//! Independent exact-identifier leg failures and empty results.

use super::{
    backends::fake,
    identifier_route::{identifier_search, identifier_search_until, publish_command},
    support::cleanup,
};
use maestro_kernel::evidence::RouteStatus;
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
async fn a_failed_kernel_leg_keeps_the_successful_payload_hit() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let connection = rusqlite::Connection::open(fixture.kernel.database_path()).unwrap();
    connection
        .execute("DROP TABLE chunk_search_fts", [])
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
