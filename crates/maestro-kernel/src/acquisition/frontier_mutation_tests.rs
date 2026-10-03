//! Frontier delegation, immutable enqueue journaling and observation attribution.
use super::{
    Frontier, Handle,
    partition_mutation_support::{Fixture, discovered, now},
};
use crate::scope::Right;

#[test]
fn k1_acknowledge_observation_requires_exact_item_and_source() {
    for (same_item, same_source) in [(true, true), (false, true), (true, false), (false, false)] {
        let fixture = Fixture::new();
        let (context, mut envelope) = fixture.capture(&discovered(0).request);
        if !same_item {
            envelope.item = Handle::new();
        }
        if !same_source {
            envelope.source = "other".into();
        }
        let artifact = fixture
            .db
            .put(&serde_json::to_vec(&envelope).unwrap(), "application/json")
            .unwrap();
        fixture
            .db
            .acknowledge(&fixture.writer, &context.item, &artifact, now())
            .unwrap();
        let observed: Option<i64> = fixture
            .db
            .reader()
            .unwrap()
            .query_row(
                "SELECT observed_ms FROM acquisition_frontier WHERE id = ?1",
                [context.item.item.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            observed,
            (same_item && same_source).then_some(i64::try_from(envelope.observed_ms).unwrap()),
            "item match {same_item}, source match {same_source}"
        );
    }
}

#[test]
fn k1_work_page_and_enqueue_journal() {
    let fixture = Fixture::new();
    let request = discovered(0).request;
    let first = fixture
        .db
        .enqueue(&fixture.writer, &request, now())
        .unwrap();
    assert_eq!(
        enqueue_events(&fixture),
        1,
        "new enqueue must append its event"
    );
    let replay = fixture
        .db
        .enqueue(&fixture.writer, &request, now())
        .unwrap();
    assert_eq!(first.id, replay.id);
    assert_eq!(
        enqueue_events(&fixture),
        1,
        "enqueue replay must not append an extra event"
    );
    fixture
        .db
        .grant("reader", &fixture.scope, Right::Read, "owner")
        .unwrap();
    let scopes = fixture.db.visible("reader").unwrap();
    let rows = fixture.db.work_page(&scopes, "docs", None, 1).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].item, first);
    assert!(!rows[0].cursor.verified);
    assert_eq!(rows[0].cursor.observed_ms, 0);
    assert!(
        fixture
            .db
            .work_page(&scopes, "docs", Some(rows[0].cursor), 1)
            .unwrap()
            .is_empty()
    );
}

/// Count only enqueue events, independent of lease/capture journaling.
fn enqueue_events(fixture: &Fixture) -> u32 {
    fixture
        .db
        .reader()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM events WHERE type = 'maestro.acquisition.enqueued.v1'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}
