//! N13 trust, bound and partial-commit guards over real kernel storage.
use super::{
    n12_support::Fixture,
    n13_durably_enumerate_public_links_and_bounded_partitions::{
        batch, capture, item, partition, run, scope,
    },
};
use maestro_acquisition::discovery::{
    links::{LinkExtraction, LinkExtractor},
    partition::discover,
};
use maestro_kernel::{
    acquisition::{
        Captures, Enumeration, Frontier, LeaseRequest, NotEnqueued, NotEnqueuedReason, Partitions,
        ReceiptError,
    },
    artifact::Digest,
};
use rusqlite::Connection;
use serde_json::json;
use std::{future::Future, pin::Pin, time::Duration};

#[test]
fn n13_batch_bounds_and_contexts_refuse_before_any_enqueue() {
    let fixture = Fixture::new();
    let mut variants = vec![];
    let valid = batch(&fixture);
    let mut bad = valid.clone();
    bad.partition.window.start = bad.partition.window.end;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.partition.max_batches = 0;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.partition.max_items = 0;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.partition.max_items = 1001;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.partition.max_batches = 1001;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.items.push(item(&fixture, "two"));
    bad.partition.max_items = 1;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.not_enqueued = vec![
        NotEnqueued {
            reference: Digest::of(b"denied"),
            reason: NotEnqueuedReason::PolicyDenial
        };
        1001
    ];
    variants.push(bad);
    let mut bad = valid.clone();
    bad.partition.kind = Enumeration::Links;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.next = Some(json!("next"));
    variants.push(bad);
    let mut bad = valid.clone();
    bad.items.first_mut().unwrap().keys.permissions = Digest::of(b"wrong");
    variants.push(bad);
    let mut bad = valid;
    bad.items.push(bad.items.first().unwrap().clone());
    variants.push(bad);
    for bad in variants {
        assert_eq!(
            fixture
                .db
                .checkpoint(&fixture.context.writer, &bad, fixture.context.now),
            Err(ReceiptError::Invalid)
        );
    }
    assert_eq!(
        fixture
            .db
            .page(&fixture.db.visible("reader").unwrap(), "notes", None, 100)
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn n13_missing_initial_cursor_and_skipped_or_changed_chain_refuse() {
    let fixture = Fixture::new();
    let mut first = batch(&fixture);
    first.items.clear();
    first.expected = Some(0);
    first.terminal = false;
    first.next = Some(json!({"token": 1}));
    let mut skipped = first.clone();
    skipped.cursor = Some(json!({"token": 3}));
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &skipped, fixture.context.now),
        Err(ReceiptError::Conflict)
    );
    fixture
        .db
        .checkpoint(&fixture.context.writer, &first, fixture.context.now)
        .unwrap();
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &skipped, fixture.context.now),
        Err(ReceiptError::Conflict)
    );
    skipped.cursor = first.next.clone();
    skipped.next = Some(json!({"token": 2}));
    skipped.partition.window.end += 1;
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &skipped, fixture.context.now),
        Err(ReceiptError::Conflict)
    );
}
#[test]
fn n13_batch_and_item_ceilings_hold_continuations() {
    for batches in [false, true] {
        let fixture = Fixture::new();
        let mut first = batch(&fixture);
        first.terminal = false;
        first.next = Some(json!(1));
        if batches {
            first.partition.max_batches = 1;
        } else {
            first.partition.max_items = 1;
        }
        fixture
            .db
            .checkpoint(&fixture.context.writer, &first, fixture.context.now)
            .unwrap();
        let mut second = first.clone();
        second.cursor = first.next.clone();
        second.next = Some(json!(2));
        second.items = vec![item(&fixture, "two")];
        assert_eq!(
            fixture
                .db
                .checkpoint(&fixture.context.writer, &second, fixture.context.now),
            Err(ReceiptError::Conflict)
        );
        assert!(
            fixture
                .db
                .partition(&scope(), first.partition.id)
                .unwrap()
                .unwrap()
                .accepted
                .is_none()
        );
    }
}
#[test]
fn n13_replay_checks_source_ownership_and_scope() {
    let fixture = Fixture::new();
    let mut batch = batch(&fixture);
    batch.items.clear();
    batch.expected = Some(0);
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
    let lease = LeaseRequest {
        holder: "other",
        now: fixture.context.now,
        term: Duration::from_secs(30),
    };
    let other = fixture.db.lease_source("other", &scope(), lease).unwrap();
    assert_eq!(
        fixture.db.checkpoint(&other, &batch, fixture.context.now),
        Err(ReceiptError::Conflict)
    );
    assert_eq!(
        fixture
            .db
            .commit_partition(&other, batch.partition.id, fixture.context.now),
        Err(ReceiptError::Invalid)
    );
    assert!(
        fixture
            .db
            .partition(
                &"workspace/default/collection/other".parse().unwrap(),
                batch.partition.id
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fixture.db.checkpoint(
            &fixture.context.writer,
            &batch,
            fixture.context.now + Duration::from_secs(31)
        ),
        Err(ReceiptError::Conflict)
    );
}
#[test]
fn n13_one_failed_item_keeps_all_pending_inventory_and_no_snapshot() {
    let fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, handle)
        .unwrap();
    let mut batch = batch(&fixture);
    batch.items.push(item(&fixture, "start"));
    batch.expected = Some(2);
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
    assert_eq!(
        fixture.db.commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now
        ),
        Err(ReceiptError::Conflict)
    );
    let saved = fixture
        .db
        .partition(&scope(), batch.partition.id)
        .unwrap()
        .unwrap();
    assert_eq!(saved.pending, 1);
    assert_eq!(saved.batches, vec![batch]);
    assert!(saved.accepted.is_none());
    let inventory = fixture
        .db
        .page(&fixture.db.visible("reader").unwrap(), "notes", None, 100)
        .unwrap();
    assert_eq!(inventory.len(), 2);
    assert_eq!(
        inventory
            .iter()
            .filter(|item| item.capture.is_none())
            .count(),
        1
    );
}
#[test]
fn n13_unknown_or_mismatched_count_evidence_holds_captured_items() {
    for expected in [None, Some(0), Some(2)] {
        let fixture = Fixture::new();
        let handle = fixture.prepare().unwrap();
        fixture
            .db
            .acknowledge_capture(&fixture.context, handle)
            .unwrap();
        let mut batch = batch(&fixture);
        batch.items = vec![item(&fixture, "start")];
        batch.expected = expected;
        fixture
            .db
            .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
            .unwrap();
        assert_eq!(
            fixture.db.commit_partition(
                &fixture.context.writer,
                batch.partition.id,
                fixture.context.now
            ),
            Err(ReceiptError::Conflict)
        );
    }
}
#[test]
fn n13_snapshot_storage_failure_never_advances_watermark() {
    let fixture = Fixture::new();
    let mut batch = batch(&fixture);
    batch.items.clear();
    batch.expected = Some(0);
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
    Connection::open(fixture.root.join("kernel.sqlite3"))
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_snapshot BEFORE INSERT ON acquisition_partition_snapshots BEGIN
         SELECT RAISE(ABORT, 'synthetic failure'); END;",
        )
        .unwrap();
    assert_eq!(
        fixture.db.commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now
        ),
        Err(ReceiptError::Storage)
    );
    assert!(
        fixture
            .db
            .partition(&scope(), batch.partition.id)
            .unwrap()
            .unwrap()
            .accepted
            .is_none()
    );
}
#[test]
fn n13_extractor_limit_hit_and_unknown_query_hold_discovery() {
    for limited in [true, false] {
        let mut fixture = Fixture::new();
        let handle = capture(&mut fixture, b"<html><a href='/docs/new'>new</a></html>");
        let result = run(discover(
            &fixture.db,
            &Limited(limited),
            (&fixture.context, handle),
            &fixture.policy,
            (partition(), 0),
        ))
        .unwrap();
        assert!(result.truncated || !result.stable);
        assert!(
            fixture
                .db
                .page(&fixture.db.visible("reader").unwrap(), "notes", None, 10)
                .unwrap()
                .iter()
                .all(|item| item.capture.is_none())
        );
    }
}
/// A substitute returns explicit parser coverage, not implied complete success.
pub(super) struct Limited(pub(super) bool);
impl LinkExtractor for Limited {
    fn contract(&self) -> &'static str {
        "synthetic bounded extractor/1"
    }
    fn extract<'a>(
        &'a self,
        _: &'a str,
        _: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<LinkExtraction, ReceiptError>> + Send + 'a>> {
        Box::pin(async {
            Ok(LinkExtraction {
                links: vec![
                    if self.0 {
                        "https://garden.example/docs/new"
                    } else {
                        "https://garden.example/docs/new?unknown=value"
                    }
                    .into(),
                ],
                contract: self.contract().into(),
                limit_hit: self.0,
            })
        })
    }
}

#[test]
fn n13_accepted_snapshot_replay_is_immutable_and_adds_no_evidence() {
    let fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, handle)
        .unwrap();
    let mut batch = batch(&fixture);
    batch.items = vec![item(&fixture, "start")];
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
    fixture
        .db
        .commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now,
        )
        .unwrap();
    let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    let before: u32 = sql
        .query_row("SELECT count(*) FROM acquisition_evidence", [], |row| {
            row.get(0)
        })
        .unwrap();
    let snapshot = fixture
        .db
        .partition(&scope(), batch.partition.id)
        .unwrap()
        .unwrap();
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
    fixture
        .db
        .commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now,
        )
        .unwrap();
    let after: u32 = sql
        .query_row("SELECT count(*) FROM acquisition_evidence", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        fixture
            .db
            .partition(&scope(), batch.partition.id)
            .unwrap()
            .unwrap(),
        snapshot
    );
    for table in [
        "acquisition_partition_batches",
        "acquisition_partition_snapshots",
    ] {
        assert!(sql.execute(&format!("DELETE FROM {table}"), []).is_err());
        assert!(
            sql.execute(&format!("UPDATE {table} SET evidence = evidence"), [])
                .is_err()
        );
    }
}
