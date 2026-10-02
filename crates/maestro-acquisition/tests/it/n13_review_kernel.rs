//! Permanent kernel regressions from the N13 review.
use super::{
    n12_support::Fixture,
    n13_durably_enumerate_public_links_and_bounded_partitions::{batch, item, scope},
};
use maestro_kernel::{
    acquisition::{
        Captures, Frontier, Handle, HeaderReason, LeaseRequest, Partitions, ReceiptError,
        SafeHeader,
    },
    artifact::Digest,
};
use serde_json::json;
use std::{
    fs,
    time::{Duration, Instant},
};

#[test]
fn n13_review_changed_index_keys_require_current_capture() {
    let fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, handle)
        .unwrap();
    let mut first = batch(&fixture);
    first.items = vec![item(&fixture, "start")];
    fixture
        .db
        .checkpoint(&fixture.context.writer, &first, fixture.context.now)
        .unwrap();
    fixture
        .db
        .commit_partition(
            &fixture.context.writer,
            first.partition.id,
            fixture.context.now,
        )
        .unwrap();
    let mut changed = first.clone();
    changed.partition.id = Handle::new();
    changed.partition.run = Handle::new();
    changed.partition.window.start = 20;
    changed.partition.window.end = 30;
    let keys = &mut changed.items.first_mut().unwrap().keys;
    keys.revision = Some(Digest::of(b"new revision"));
    keys.validator = Some(Digest::of(b"new validator"));
    keys.links = Digest::of(b"new links");
    keys.representation = Digest::of(b"new representation");
    fixture
        .db
        .checkpoint(&fixture.context.writer, &changed, fixture.context.now)
        .unwrap();
    assert_eq!(
        fixture.db.commit_partition(
            &fixture.context.writer,
            changed.partition.id,
            fixture.context.now
        ),
        Err(ReceiptError::Conflict)
    );
    let state = fixture
        .db
        .partition(&scope(), changed.partition.id)
        .unwrap()
        .unwrap();
    assert_eq!(state.pending, 1);
    assert!(state.accepted.is_none());
    assert_eq!(state.batches, vec![changed]);
}

#[test]
fn n13_review_each_unproven_key_holds() {
    let fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, handle)
        .unwrap();
    for key in ["revision", "validator", "representation"] {
        let mut batch = batch(&fixture);
        batch.items = vec![item(&fixture, "start")];
        let keys = &mut batch.items.first_mut().unwrap().keys;
        match key {
            "revision" => keys.revision = Some(Digest::of(b"unknown revision")),
            "validator" => keys.validator = Some(Digest::of(b"wrong validator")),
            _ => keys.representation = Digest::of(b"wrong body"),
        }
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
            Err(ReceiptError::Conflict),
            "{key}"
        );
        assert_eq!(
            fixture
                .db
                .partition(&scope(), batch.partition.id)
                .unwrap()
                .unwrap()
                .pending,
            1,
            "{key}"
        );
    }
}

#[test]
fn n13_review_absent_validator_claim_with_headers_commits() {
    let mut fixture = Fixture::new();
    fixture.envelope.headers.insert(
        "etag".into(),
        SafeHeader::Hashed {
            digest: Digest::of(b"validator"),
            reason: HeaderReason::UntrustedValue,
        },
    );
    let handle = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, handle)
        .unwrap();
    let mut batch = batch(&fixture);
    batch.items = vec![item(&fixture, "start")];
    batch.items.first_mut().unwrap().keys.validator = None;
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
        Ok(())
    );
}

#[test]
fn n13_review_accepted_replay_checks_source() {
    let fixture = Fixture::new();
    let mut batch = batch(&fixture);
    batch.items.clear();
    batch.expected = Some(0);
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
    let other = fixture
        .db
        .lease_source(
            "other",
            &scope(),
            LeaseRequest {
                holder: "other",
                now: fixture.context.now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    assert_eq!(
        fixture
            .db
            .commit_partition(&other, batch.partition.id, fixture.context.now),
        Err(ReceiptError::Invalid)
    );
}

#[test]
fn n13_review_readback_refuses_real_file_above_cap() {
    let fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    assert_eq!(
        fixture
            .db
            .read_capture(&fixture.context, handle, 4)
            .unwrap()
            .1
            .unwrap(),
        b"body"
    );
    let digest = fixture.envelope.artifact.as_str();
    fs::write(
        fixture
            .root
            .join("artifacts/sha256")
            .join(digest.get(..2).unwrap())
            .join(digest.get(2..4).unwrap())
            .join(digest),
        vec![b'x'; 8192],
    )
    .unwrap();
    assert_eq!(
        fixture.db.read_capture(&fixture.context, handle, 1000),
        Err(ReceiptError::Invalid)
    );
}

#[test]
fn n13_review_reference_ceiling_refuses_first_checkpoint() {
    let fixture = Fixture::new();
    let mut batch = batch(&fixture);
    batch.partition.max_batches = 1000;
    batch.partition.max_items = 1;
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &batch, fixture.context.now),
        Err(ReceiptError::Invalid)
    );
    assert!(
        fixture
            .db
            .partition(&scope(), batch.partition.id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn n13_review_distinct_item_fits_overlapping_continuation() {
    let fixture = Fixture::new();
    let mut first = batch(&fixture);
    first.partition.max_items = 1;
    first.terminal = false;
    first.next = Some(json!(1));
    fixture
        .db
        .checkpoint(&fixture.context.writer, &first, fixture.context.now)
        .unwrap();
    let mut second = first.clone();
    second.cursor = first.next.clone();
    second.next = None;
    second.terminal = true;
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &second, fixture.context.now),
        Ok(())
    );
    let state = fixture
        .db
        .partition(&scope(), first.partition.id)
        .unwrap()
        .unwrap();
    assert_eq!(state.pending, 1);
    assert_eq!(state.batches, vec![first, second]);
}

#[test]
fn n13_review_largest_legal_declaration_commits() {
    let fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, handle)
        .unwrap();
    let mut batch = batch(&fixture);
    batch.partition.max_batches = 999;
    batch.partition.max_items = 1;
    batch.items = vec![item(&fixture, "start")];
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
        Ok(())
    );
}

#[test]
fn n13_review_one_hundred_checkpoint_timing() {
    let fixture = Fixture::new();
    let mut batch = batch(&fixture);
    batch.partition.max_batches = 100;
    batch.items.clear();
    batch.expected = Some(0);
    let started = Instant::now();
    for index in 0..100 {
        batch.cursor = (index > 0).then(|| json!(index));
        batch.next = (index < 99).then(|| json!(index + 1));
        batch.terminal = index == 99;
        fixture
            .db
            .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
            .unwrap();
    }
    eprintln!("100 checkpoints: {:?}", started.elapsed());
    fixture
        .db
        .commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now,
        )
        .unwrap();
    let state = fixture
        .db
        .partition(&scope(), batch.partition.id)
        .unwrap()
        .unwrap();
    assert_eq!(state.batches.len(), 100);
    assert_eq!(state.accepted.unwrap().watermark, 20);
}
