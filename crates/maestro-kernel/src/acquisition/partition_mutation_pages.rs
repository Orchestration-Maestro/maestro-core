//! Read ports return exact persisted identities and first-depth provenance.
use super::{
    Captures, DepthPage, Partitions, ReceiptError,
    partition_mutation_support::{Fixture, batch, discovered, now},
};
use rusqlite::params;

#[test]
fn k1_partition_summary_and_identity_pages() {
    let fixture = Fixture::new();
    let pending = batch();
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &pending, now())
            .is_ok()
    );
    let summaries = fixture.db.summary_page(&fixture.scope, "docs", None, 1000);
    assert!(summaries.is_ok(), "valid summary page refused");
    let summaries = summaries.unwrap();
    assert_eq!(summaries.len(), 1);
    let summary = &summaries[0];
    assert_eq!(summary.id, pending.partition.id);
    assert_eq!(summary.run, pending.partition.run);
    assert_eq!(summary.window, pending.partition.window);
    assert_eq!(summary.sequence, 0);
    assert!(summary.committable);
    assert!(!summary.accepted);
    assert!(
        fixture
            .db
            .partition_page(&fixture.scope, "docs", None, 1000)
            .is_ok_and(|rows| rows == vec![pending.partition.id])
    );
    assert!(
        fixture
            .db
            .partition_page(&fixture.scope, "docs", Some(pending.partition.id), 1)
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .db
            .summary_page(&fixture.scope, "docs", Some((pending.partition.id, 0)), 1)
            .unwrap()
            .is_empty()
    );
    fixture
        .db
        .commit_partition(&fixture.writer, pending.partition.id, now())
        .unwrap();
    assert!(
        fixture
            .db
            .summary_page(&fixture.scope, "docs", None, 1)
            .unwrap()[0]
            .accepted
    );
}

#[test]
fn k1_partition_page_limits() {
    let fixture = Fixture::new();
    for limit in [0, 1001] {
        assert!(matches!(
            fixture
                .db
                .partition_page(&fixture.scope, "docs", None, limit),
            Err(ReceiptError::Invalid)
        ));
        assert!(matches!(
            fixture.db.summary_page(&fixture.scope, "docs", None, limit),
            Err(ReceiptError::Invalid)
        ));
    }
}

#[test]
fn k1_depth_page_returns_exact_indexed_evidence() {
    let fixture = Fixture::new();
    let item = discovered(0);
    let (context, envelope) = fixture.capture(&item.request);
    let capture = fixture
        .db
        .prepare_capture(&context, &envelope, b"body", u64::MAX)
        .unwrap()
        .handle;
    // Seed a persisted index row to isolate read-port behavior from history recording.
    let pending = batch();
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &pending, now())
            .is_ok()
    );
    fixture
        .db
        .write::<_, ReceiptError>(|tx| {
            tx.execute(
                "INSERT INTO acquisition_depths (source, scope, fetch_identity,
            authorization_context, representation_profile, partition, sequence, ordinal,
            depth, capture) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, '7', ?7)",
                params![
                    "docs",
                    fixture.scope.as_str(),
                    item.request.fetch_identity,
                    item.request.authorization_context.as_str(),
                    item.request.representation_profile.as_str(),
                    pending.partition.id.to_string(),
                    capture.to_string()
                ],
            )?;
            Ok(())
        })
        .unwrap();
    let mut page = DepthPage {
        source: "docs",
        scope: &fixture.scope,
        authorization: &item.request.authorization_context,
        profile: &item.request.representation_profile,
        after: None,
        limit: 1000,
    };
    let rows = fixture.db.depth_page(&page);
    assert!(rows.is_ok(), "valid depth page refused");
    let rows = rows.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].request, item.request);
    assert_eq!(rows[0].capture, capture);
    assert_eq!(rows[0].depth, 7);
    page.after = Some(&item.request.fetch_identity);
    assert!(fixture.db.depth_page(&page).unwrap().is_empty());
    for limit in [0, 1001] {
        page.limit = limit;
        assert!(matches!(
            fixture.db.depth_page(&page),
            Err(ReceiptError::Invalid)
        ));
    }
}
