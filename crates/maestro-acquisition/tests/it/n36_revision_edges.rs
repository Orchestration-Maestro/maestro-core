//! N36 observation provenance and finite durable integer bounds.
use super::n12_support::Fixture;
use maestro_kernel::acquisition::{Error, Frontier, Handle};

#[test]
fn n36_acknowledged_observation_requires_matching_source_and_item() {
    for mismatch in 0..3 {
        let fixture = Fixture::new();
        let mut envelope = fixture.envelope.clone();
        match mismatch {
            0 => envelope.source = "other".into(),
            1 => envelope.item = Handle::new(),
            _ => {}
        }
        let digest = fixture
            .db
            .put(&serde_json::to_vec(&envelope).unwrap(), "application/json")
            .unwrap();
        fixture
            .db
            .acknowledge(
                &fixture.context.writer,
                &fixture.context.item,
                &digest,
                fixture.context.now,
            )
            .unwrap();
        let row = fixture
            .db
            .work_page(&fixture.db.visible("reader").unwrap(), "notes", None, 1)
            .unwrap()
            .remove(0);
        assert_eq!(
            row.cursor.observed_ms,
            if mismatch == 2 {
                fixture.envelope.observed_ms
            } else {
                0
            },
            "foreign envelope supplied observation"
        );
    }
}

#[test]
fn n36_out_of_range_observation_does_not_acknowledge_frontier() {
    let mut fixture = Fixture::new();
    fixture.envelope.observed_ms = u64::MAX;
    let digest = fixture
        .db
        .put(
            &serde_json::to_vec(&fixture.envelope).unwrap(),
            "application/json",
        )
        .unwrap();
    assert!(matches!(
        fixture.db.acknowledge(
            &fixture.context.writer,
            &fixture.context.item,
            &digest,
            fixture.context.now
        ),
        Err(Error::Invalid)
    ));
    let row = fixture
        .db
        .work_page(&fixture.db.visible("reader").unwrap(), "notes", None, 1)
        .unwrap()
        .remove(0);
    assert!(
        row.item.capture.is_none(),
        "invalid observation marked visited"
    );
}
