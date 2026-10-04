//! N36 local verification windows and append-only source revalidation.
use super::{
    n12_support::Fixture,
    n13_durably_enumerate_public_links_and_bounded_partitions::{batch, scope},
};
use maestro_acquisition::lifecycle::{
    full::{Mode, changed, same_capture},
    incremental::{due, window},
};
use maestro_kernel::{
    acquisition::{
        CaptureContext, Captures, DispatchRequest, Frontier, Handle, Item, LeaseRequest,
        Partitions, ReceiptError, Receipts, Representation, SafeHeader, SafeIdentity,
    },
    artifact::Digest,
};
use std::time::Duration;

#[test]
fn n36_full_and_overlapping_skewed_incremental_windows() {
    let bounds = window(Some(100), 120, 10, 5).unwrap();
    assert_eq!(
        (bounds.start, bounds.end, bounds.overlap, bounds.skew),
        (85, 120, 10, 5)
    );
    assert!(due(Mode::Full, Some(100), Some(110), &bounds));
    assert!(due(Mode::Incremental, None, Some(110), &bounds));
    assert!(due(Mode::Incremental, Some(100), None, &bounds));
    assert!(due(Mode::Incremental, Some(100), Some(84), &bounds));
    assert!(!due(Mode::Incremental, Some(100), Some(85), &bounds));
    assert_eq!(window(Some(5), 10, 10, 5).unwrap().start, 0);
    assert!(window(Some(121), 120, 0, 0).is_err());
    assert!(window(None, 120, u64::MAX, 1).is_err());
}

#[test]
fn n36_metadata_permission_link_and_reply_changes_are_not_text_equality() {
    let fixture = Fixture::new();
    let mut before = batch(&fixture).items.remove(0).keys;
    before.revision = Some(Digest::of(b"original source revision"));
    for field in 0..6 {
        let mut after = before.clone();
        let digest = Digest::of(b"edited or deleted reply, unchanged visible parent text");
        match field {
            0 => after.revision = Some(digest),
            1 => after.validator = Some(digest),
            2 => after.metadata = Some(digest),
            3 => after.permissions = digest,
            4 => after.links = Some(digest),
            _ => after.representation = Some(digest),
        }
        assert!(changed(&before, &after), "source signal {field} was hidden");
    }
    assert!(!changed(&before, &before));
}

#[test]
fn n36_refresh_appends_changed_captures_and_fences_old_writers() {
    let mut fixture = Fixture::new();
    let original = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, original)
        .unwrap();
    let now = fixture.context.now;
    let writer = &fixture.context.writer;
    fixture
        .db
        .refresh(writer, fixture.context.item.item, now)
        .unwrap();
    assert!(
        fixture
            .db
            .refresh(writer, fixture.context.item.item, now)
            .is_err(),
        "pending row refreshed twice"
    );
    let row = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        1,
    )
    .unwrap()
    .remove(0);
    assert_eq!(
        fixture.db.prepared_for(&scope(), &row).unwrap(),
        None,
        "old generation became prepared evidence"
    );
    let dispatch = fixture
        .db
        .lease(
            writer,
            fixture.context.item.item,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "worker",
                    now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 10,
            },
        )
        .unwrap();
    let old = fixture.context.clone();
    fixture.context.item = dispatch;
    fixture.envelope.artifact = Digest::of(b"edit");
    let updated = fixture
        .db
        .prepare_capture(&fixture.context, &fixture.envelope, b"edit", u64::MAX)
        .unwrap()
        .handle;
    assert_ne!(updated, original);
    assert!(
        fixture
            .db
            .acknowledge_capture(&fixture.context, original)
            .is_err(),
        "prior generation acknowledged under fresh lease"
    );
    assert!(
        fixture.db.acknowledge_capture(&old, original).is_err(),
        "stale dispatch accepted"
    );
    fixture
        .db
        .acknowledge_capture(&fixture.context, updated)
        .unwrap();
    assert!(
        fixture.db.read("reader", original).unwrap().is_some(),
        "old immutable capture lost"
    );
    let rows = fixture
        .db
        .work_page(&fixture.db.visible("reader").unwrap(), "notes", None, 1)
        .unwrap();
    assert_eq!(
        rows.first().unwrap().cursor.observed_ms,
        fixture.envelope.observed_ms
    );
    assert_eq!(
        fixture
            .db
            .capture_for(&scope(), &rows.first().unwrap().item)
            .unwrap(),
        Some(updated)
    );
    fixture.db.release_source(writer, now).unwrap();
    assert!(
        fixture.db.refresh(writer, old.item.item, now).is_err(),
        "released source refreshed"
    );
}

#[test]
fn n36_capped_unsplittable_unstable_windows_keep_actual_coverage() {
    for (stable, truncated, expected, terminal) in [
        (true, false, Some(0), true),
        (true, true, Some(0), true),
        (false, false, Some(0), true),
        (true, false, None, true),
        (true, false, Some(0), false),
    ] {
        let fixture = Fixture::new();
        let mut checkpoint = batch(&fixture);
        checkpoint.items.clear();
        checkpoint.stable = stable;
        checkpoint.truncated = truncated;
        checkpoint.expected = expected;
        checkpoint.terminal = terminal;
        checkpoint.next = (!terminal).then(|| serde_json::json!({"continuation": [1, 2]}));
        fixture
            .db
            .checkpoint(&fixture.context.writer, &checkpoint, fixture.context.now)
            .unwrap();
        let result = fixture.db.commit_partition(
            &fixture.context.writer,
            checkpoint.partition.id,
            fixture.context.now,
        );
        if stable && !truncated && expected.is_some() && terminal {
            result.unwrap();
        } else {
            assert_eq!(
                result,
                Err(ReceiptError::Conflict),
                "incomplete window committed"
            );
        }
        let saved = fixture
            .db
            .partition(&scope(), checkpoint.partition.id)
            .unwrap()
            .unwrap();
        assert_eq!(saved.batches, vec![checkpoint]);
        assert_eq!(
            saved.accepted.is_some(),
            stable && !truncated && expected.is_some() && terminal
        );
    }
}

#[test]
fn n36_oldest_outstanding_cursor_pages_pending_before_verified() {
    let fixture = Fixture::new();
    let capture = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, capture)
        .unwrap();
    let source_item = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        1,
    )
    .unwrap()
    .remove(0);
    let mut request = source_item.request;
    request.fetch_identity = "https://garden.example/docs/new".into();
    let pending = fixture
        .db
        .enqueue(&fixture.context.writer, &request, fixture.context.now)
        .unwrap();
    let scopes = fixture.db.visible("reader").unwrap();
    let first = fixture
        .db
        .work_page(&scopes, "notes", None, 1)
        .unwrap()
        .remove(0);
    assert_eq!(
        first.item.id, pending.id,
        "new pending must precede old acknowledged row"
    );
    let older = verified(&fixture, "older", 100);
    let newer = verified(&fixture, "newer", 1_000_000_001);
    let second = fixture
        .db
        .work_page(&scopes, "notes", Some(first.cursor), 1)
        .unwrap()
        .remove(0);
    assert_eq!(
        second.item.id, older.id,
        "oldest observation must precede older durable IDs"
    );
    let third = fixture
        .db
        .work_page(&scopes, "notes", Some(second.cursor), 1)
        .unwrap()
        .remove(0);
    assert_eq!(third.item.id, fixture.context.item.item);
    let fourth = fixture
        .db
        .work_page(&scopes, "notes", Some(third.cursor), 1)
        .unwrap()
        .remove(0);
    assert_eq!(fourth.item.id, newer.id);
    assert!(
        fixture
            .db
            .work_page(&scopes, "notes", Some(fourth.cursor), 1)
            .unwrap()
            .is_empty()
    );
    assert!(fixture.db.work_page(&scopes, "notes", None, 0).is_err());
    assert!(
        fixture
            .db
            .work_page(&fixture.db.visible("denied").unwrap(), "notes", None, 1)
            .unwrap()
            .is_empty()
    );
}

/// Independently observed captures intentionally disagree with durable ID order.
fn verified(fixture: &Fixture, suffix: &str, observed: u64) -> Item {
    let mut request = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        1,
    )
    .unwrap()
    .remove(0)
    .request;
    request.fetch_identity = format!("https://garden.example/docs/{suffix}");
    let writer = &fixture.context.writer;
    let now = fixture.context.now;
    let item = fixture.db.enqueue(writer, &request, now).unwrap();
    let dispatch = fixture
        .db
        .lease(
            writer,
            item.id,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "worker",
                    now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 3,
            },
        )
        .unwrap();
    let context = CaptureContext {
        writer: writer.clone(),
        item: dispatch,
        now,
    };
    let mut envelope = fixture.envelope.clone();
    envelope.item = item.id.to_string().parse().unwrap();
    envelope.requested = SafeIdentity::new(&request.fetch_identity).unwrap();
    envelope.final_identity = envelope.requested.clone();
    envelope.observed_ms = observed;
    let capture = fixture
        .db
        .prepare_capture(&context, &envelope, b"body", u64::MAX)
        .unwrap();
    fixture
        .db
        .acknowledge_capture(&context, capture.handle)
        .unwrap();
    item
}

#[test]
fn n36_http_semantic_comparison_excludes_local_observations_only() {
    let fixture = Fixture::new();
    let before = &fixture.envelope;
    for signal in 0..9 {
        let mut after = before.clone();
        match signal {
            0 => after.artifact = Digest::of(b"reply edit or deletion"),
            1 => {
                after.headers.insert(
                    "etag".into(),
                    SafeHeader::Value {
                        value: "synthetic validator".into(),
                    },
                );
            }
            2 => after.declared_media = Some("text/html".into()),
            3 => after.detected_media = Some("text/html".into()),
            4 => after.authorization_context = Digest::of(b"narrower permissions"),
            5 => after.profile = Digest::of(b"new representation profile"),
            6 => {
                after.final_identity =
                    SafeIdentity::new("https://garden.example/docs/other").unwrap();
            }
            7 => after.status = 201,
            _ => after.representation = Representation::ApiRecord,
        }
        assert!(
            !same_capture(before, &after),
            "HTTP source signal {signal} was hidden"
        );
    }
    let mut observation = before.clone();
    observation.run = Handle::new();
    observation.observed_ms += 100;
    assert!(
        same_capture(before, &observation),
        "local run or time caused semantic revision churn"
    );
}

#[test]
fn n36_equal_bytes_refresh_cannot_acknowledge_a_prior_generation_handle() {
    let mut fixture = Fixture::new();
    assert!(
        fixture
            .db
            .refresh(
                &fixture.context.writer,
                "00000000000000000000000000".parse().unwrap(),
                fixture.context.now
            )
            .is_err(),
        "unknown item refreshed"
    );
    let original = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, original)
        .unwrap();
    let foreign = fixture
        .db
        .lease_source(
            "other",
            &scope(),
            LeaseRequest {
                holder: "worker",
                now: fixture.context.now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    assert!(
        fixture
            .db
            .refresh(&foreign, fixture.context.item.item, fixture.context.now)
            .is_err(),
        "foreign source refreshed item"
    );
    fixture
        .db
        .release_source(&foreign, fixture.context.now)
        .unwrap();
    fixture
        .db
        .refresh(
            &fixture.context.writer,
            fixture.context.item.item,
            fixture.context.now,
        )
        .unwrap();
    fixture.context.item = fixture
        .db
        .lease(
            &fixture.context.writer,
            fixture.context.item.item,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "worker",
                    now: fixture.context.now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 10,
            },
        )
        .unwrap();
    let current = fixture.prepare().unwrap();
    let row = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        1,
    )
    .unwrap()
    .remove(0);
    assert_eq!(
        fixture.db.prepared_for(&scope(), &row).unwrap(),
        Some(current)
    );
    let other_scope = "workspace/default/collection/other".parse().unwrap();
    assert_eq!(
        fixture.db.prepared_for(&other_scope, &row).unwrap(),
        None,
        "prepared evidence crossed collection scopes"
    );
    assert_ne!(original, current);
    assert!(
        fixture
            .db
            .acknowledge_capture(&fixture.context, original)
            .is_err(),
        "old equal-byte handle crossed generations"
    );
    fixture
        .db
        .acknowledge_capture(&fixture.context, current)
        .unwrap();
}

#[test]
fn n36_work_cursor_seeks_the_composite_index_without_a_temporary_sort() {
    let fixture = Fixture::new();
    let connection = rusqlite::Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    let sql = "EXPLAIN QUERY PLAN SELECT id FROM acquisition_frontier
        WHERE source = 'notes' AND (work_verified, work_observed, id) > (1, 100, 'cursor')
        ORDER BY work_verified, work_observed, id LIMIT 2";
    let plan: Vec<String> = connection
        .prepare(sql)
        .unwrap()
        .query_map([], |row| row.get(3))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(
        plan.iter()
            .any(|line| line.contains("acquisition_frontier_work_page")
                && line.contains("(work_verified,work_observed,id)>(?,?,?)")),
        "cursor wasn't an indexed range: {plan:?}"
    );
    assert!(
        !plan.iter().any(|line| line.contains("TEMP B-TREE")),
        "frontier sort: {plan:?}"
    );
}
