//! Summary-index and batch-lookup parity against authoritative immutable evidence.
use super::flow_edges::{Fixture, clean};
use maestro_acquisition::lifecycle::full::Mode;
use maestro_kernel::{
    acquisition::{
        Batch, CaptureEnvelope, Captures, DepthPage, Frontier, LeaseRequest, PartitionSummary,
        Partitions, ReceiptError, Receipts, Status,
    },
    artifact::Digest,
};
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn n36_scale_batch_capture_lookup_preserves_scope_generation_and_bytes() {
    let fixture = Fixture::new(clean);
    let report = fixture.sync_window(Mode::Full, UNIX_EPOCH + Duration::from_secs(2_000_000));
    assert_eq!(report.status, Status::Complete);
    let items = Frontier::page(&fixture.db, &fixture.scopes, "notes", None, 100).unwrap();
    let before = fixture.db.statement_count();
    let page = fixture.db.capture_page(&fixture.scope, &items).unwrap();
    assert_eq!(
        fixture.db.statement_count() - before,
        1,
        "batch lookup issued per-item SQL"
    );
    assert_eq!(page.len(), items.len());
    for item in &items {
        let observed = page.get(&item.id).unwrap();
        assert!(observed.acknowledged);
        assert_eq!(
            fixture.db.capture_for(&fixture.scope, item).unwrap(),
            Some(observed.handle)
        );
        let bytes = fixture.db.read("reader", observed.handle).unwrap().unwrap();
        let envelope: CaptureEnvelope = serde_json::from_slice(bytes.bytes()).unwrap();
        assert_eq!(envelope, observed.envelope);
    }
    let foreign = "workspace/default/collection/other".parse().unwrap();
    assert!(
        fixture
            .db
            .capture_page(&foreign, &items)
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .db
            .capture_page(&fixture.scope, &[])
            .unwrap()
            .is_empty()
    );
    let oversized = vec![items.first().unwrap().clone(); 1001];
    assert_eq!(
        fixture
            .db
            .capture_page(&fixture.scope, &oversized)
            .unwrap_err(),
        ReceiptError::Invalid
    );
    let now = SystemTime::now();
    let writer = fixture
        .db
        .lease_source(
            "notes",
            &fixture.scope,
            LeaseRequest {
                holder: "refresh",
                now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    fixture
        .db
        .refresh(&writer, items.first().unwrap().id, now)
        .unwrap();
    let fresh = fixture.db.capture_page(&fixture.scope, &items).unwrap();
    assert!(
        !fresh.contains_key(&items.first().unwrap().id),
        "old generation leaked into batch lookup"
    );
    fixture.db.release_source(&writer, now).unwrap();
    fixture.finish();
}

#[test]
fn n36_scale_summary_and_first_depth_match_old_history_walk() {
    let fixture = Fixture::new(clean);
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    for index in 0..3 {
        assert_eq!(
            fixture
                .sync_window(Mode::Full, now + Duration::from_secs(index))
                .status,
            Status::Complete
        );
    }
    // A later-inserted but earlier-ID checkpoint reaches the same children at another depth.
    // Recovery historically uses ID order, not minimum depth or insertion order.
    let ids = fixture
        .db
        .partition_page(&fixture.scope, "notes", None, 100)
        .unwrap();
    let mut root = ids
        .iter()
        .rev()
        .find_map(|id| {
            fixture
                .db
                .partition(&fixture.scope, *id)
                .unwrap()
                .unwrap()
                .batches
                .into_iter()
                .find(|batch| !batch.items.is_empty() && batch.capture.is_some())
        })
        .unwrap();
    root.partition.id = "00000000000000000000000001".parse().unwrap();
    root.parent_depth = Some(2);
    root.partition.max_batches = 2;
    root.partition.max_items = 998;
    root.terminal = false;
    root.next = Some(serde_json::json!("second"));
    let at = SystemTime::now();
    let writer = fixture
        .db
        .lease_source(
            "notes",
            &fixture.scope,
            LeaseRequest {
                holder: "parity",
                now: at,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    fixture.db.checkpoint(&writer, &root, at).unwrap();
    root.cursor = root.next.take();
    root.terminal = true;
    fixture.db.checkpoint(&writer, &root, at).unwrap();
    fixture.db.release_source(&writer, at).unwrap();
    let (old, all) = old_history(&fixture);
    let indexed: BTreeMap<_, _> = fixture
        .db
        .depth_page(&DepthPage {
            source: "notes",
            scope: &fixture.scope,
            authorization: &Digest::of(b"reader"),
            profile: &fixture
                .policy
                .policy()
                .sources
                .first()
                .unwrap()
                .acquisition_profile
                .digest,
            after: None,
            limit: 100,
        })
        .unwrap()
        .into_iter()
        .map(|item| (item.request.fetch_identity, item.depth))
        .collect();
    assert_eq!(old, indexed);
    assert_eq!(indexed.get("https://garden.example/docs/allowed"), Some(&3));
    // Page every summary at size one, including multi-batch cursor correctness.
    let mut paged = Vec::new();
    let mut after = None;
    loop {
        let page = fixture
            .db
            .summary_page(&fixture.scope, "notes", after, 1)
            .unwrap();
        if page.is_empty() {
            break;
        }
        after = page.last().map(|summary| (summary.id, summary.sequence));
        paged.extend(page);
    }
    assert_eq!(paged, all);
    fixture.finish();
}

/// Independent original full-inventory walk also proves every derived summary equals its batch.
fn old_history(fixture: &Fixture) -> (BTreeMap<String, u64>, Vec<PartitionSummary>) {
    let mut old = BTreeMap::new();
    let ids = fixture
        .db
        .partition_page(&fixture.scope, "notes", None, 100)
        .unwrap();
    let all = fixture
        .db
        .summary_page(&fixture.scope, "notes", None, 100)
        .unwrap();
    for id in ids {
        let state = fixture.db.partition(&fixture.scope, id).unwrap().unwrap();
        for (sequence, batch) in state.batches.iter().enumerate() {
            let summary = all
                .iter()
                .find(|summary| summary.id == id && summary.sequence as usize == sequence)
                .unwrap();
            assert_eq!(summary.run, batch.partition.run);
            assert_eq!(summary.kind, batch.partition.kind);
            assert_eq!(summary.window, batch.partition.window);
            assert_eq!(summary.verification_final, batch.verification_final);
            assert_eq!(summary.capture, batch.capture);
            assert_eq!(summary.committable, batch.stable && !batch.truncated);
            assert_eq!(summary.accepted, state.accepted.is_some());
            old_depths(fixture, batch, &mut old);
        }
    }
    (old, all)
}
/// Exact former parent-then-children insertion order, without minimum-depth inference.
fn old_depths(fixture: &Fixture, batch: &Batch, old: &mut BTreeMap<String, u64>) {
    let (Some(depth), Some(handle)) = (batch.parent_depth, batch.capture) else {
        return;
    };
    let bytes = fixture.db.read("reader", handle).unwrap().unwrap();
    let envelope: CaptureEnvelope = serde_json::from_slice(bytes.bytes()).unwrap();
    old.entry(envelope.requested.as_str().to_owned())
        .or_insert(depth);
    for item in &batch.items {
        old.entry(item.request.fetch_identity.clone())
            .or_insert(depth.saturating_add(1));
    }
}
