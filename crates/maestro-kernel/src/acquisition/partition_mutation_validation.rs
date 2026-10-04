//! Independent validation and coverage boundaries, including equality cases.
use super::{
    Enumeration, NotEnqueued, NotEnqueuedReason, Partitions, ReceiptError,
    partition_mutation_support::{Fixture, batch, discovered, now},
};
use crate::artifact::Digest;

#[test]
fn k1_checkpoint_validation_boundaries() {
    let fixture = Fixture::new();
    let valid = batch();
    let mut cases = vec![];
    let mut changed = valid.clone();
    changed.partition.window.start = 21;
    cases.push(changed);
    let mut changed = valid.clone();
    changed.partition.window.end = 10;
    cases.push(changed);
    let mut changed = valid.clone();
    changed.partition.max_batches = 0;
    cases.push(changed);
    let mut changed = valid.clone();
    changed.partition.max_items = 0;
    cases.push(changed);
    let mut changed = valid.clone();
    changed.partition.max_batches = 600;
    changed.partition.max_items = 500;
    cases.push(changed);
    let mut changed = valid.clone();
    changed.partition.max_batches = 1;
    changed.partition.max_items = 1000;
    cases.push(changed);
    let mut changed = valid.clone();
    changed.items = (0..3).map(discovered).collect();
    cases.push(changed);
    let mut changed = valid.clone();
    changed.inventory_overflow = 1;
    cases.push(changed);
    let mut changed = valid.clone();
    changed.not_enqueued = vec![discarded(); 1001];
    cases.push(changed);
    let mut changed = valid.clone();
    changed.partition.kind = Enumeration::Links;
    changed.extractor = Some("links/1".into());
    cases.push(changed);
    let mut changed = valid.clone();
    changed.next = Some(serde_json::json!(1));
    cases.push(changed);
    let mut changed = valid.clone();
    let mut item = discovered(0);
    item.keys.permissions = Digest::of(b"wrong");
    changed.items = vec![item];
    cases.push(changed);
    let mut changed = valid;
    changed.items = vec![discovered(0); 2];
    cases.push(changed);
    for (index, changed) in cases.iter().enumerate() {
        assert!(
            matches!(
                fixture.db.checkpoint(&fixture.writer, changed, now()),
                Err(ReceiptError::Invalid)
            ),
            "invalid case {index}"
        );
    }
}

#[test]
fn k1_checkpoint_valid_equalities() {
    let fixture = Fixture::new();
    let mut valid = batch();
    valid.partition.kind = Enumeration::Verification;
    valid.partition.window.end = valid.partition.window.start;
    valid.partition.max_batches = 1;
    valid.partition.max_items = 999;
    valid.items = (0..999).map(discovered).collect();
    valid.not_enqueued = vec![discarded(); 1000];
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &valid, now())
            .is_ok()
    );
    let mut overflow = batch();
    overflow.inventory_overflow = 1;
    overflow.truncated = true;
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &overflow, now())
            .is_ok()
    );
}

#[test]
fn k1_complete_independent_coverage_claims() {
    let fixture = Fixture::new();
    let mut cases = vec![];
    let mut changed = batch();
    changed.stable = false;
    cases.push(changed);
    let mut changed = batch();
    changed.not_enqueued = vec![NotEnqueued {
        reference: Digest::of(b"unknown"),
        reason: NotEnqueuedReason::UnresolvedIdentity,
    }];
    cases.push(changed);
    let mut changed = batch();
    changed.truncated = true;
    cases.push(changed);
    let mut changed = batch();
    changed.expected = None;
    cases.push(changed);
    for (index, changed) in cases.iter().enumerate() {
        assert!(
            fixture
                .db
                .checkpoint(&fixture.writer, changed, now())
                .is_ok()
        );
        assert!(
            matches!(
                fixture
                    .db
                    .commit_partition(&fixture.writer, changed.partition.id, now()),
                Err(ReceiptError::Conflict)
            ),
            "incomplete case {index}"
        );
    }
    let mut valid = batch();
    valid.not_enqueued = vec![discarded()];
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &valid, now())
            .is_ok()
    );
    assert!(
        fixture
            .db
            .commit_partition(&fixture.writer, valid.partition.id, now())
            .is_ok()
    );
    assert!(
        fixture
            .db
            .partition(&fixture.scope, valid.partition.id)
            .unwrap()
            .unwrap()
            .accepted
            .is_some()
    );
}

/// Discarded references must not make coverage pending.
fn discarded() -> NotEnqueued {
    NotEnqueued {
        reference: Digest::of(b"denied"),
        reason: NotEnqueuedReason::PolicyDenial,
    }
}
