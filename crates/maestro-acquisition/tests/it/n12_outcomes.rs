//! Distinct stage inventories reconcile independently of dispatch attempts.
#![expect(
    clippy::indexing_slicing,
    reason = "authored synthetic inventory positions"
)]
use maestro_acquisition::capture::{
    CaptureContext, CaptureEnvelope, Captures, Outcome, reconcile as reconcile_verified,
};
use maestro_kernel::acquisition::{
    Handle, InventoryPage, InventorySchema, Item, ItemDisposition, NewItem, PreparedCapture,
    Reason, ReceiptError, Stage, StageItem, Status,
};
use maestro_kernel::{artifact::Digest, scope::Scope};

use std::slice;

#[test]
fn n12_incomplete_blocked_and_failed_are_never_success() {
    for (complete, disposition, reason, status) in [
        (
            false,
            ItemDisposition::Accepted,
            Reason::None,
            Status::Partial,
        ),
        (
            true,
            ItemDisposition::Pending,
            Reason::None,
            Status::Partial,
        ),
        (
            true,
            ItemDisposition::Blocked,
            Reason::None,
            Status::Blocked,
        ),
        (
            true,
            ItemDisposition::Refused,
            Reason::Transport,
            Status::Failed,
        ),
        (
            true,
            ItemDisposition::Refused,
            Reason::None,
            Status::Partial,
        ),
        (
            true,
            ItemDisposition::Accepted,
            Reason::Authentication,
            Status::Blocked,
        ),
        (
            false,
            ItemDisposition::Accepted,
            Reason::Authentication,
            Status::Partial,
        ),
    ] {
        let page = page(complete, disposition);
        let frontier = frontier(&page, 7);
        let outcome = reconcile(&[page], &[frontier], reason).unwrap();
        assert_eq!(outcome.status, status);
        assert!(!outcome.success());
        assert_eq!(outcome.attempts, 7);
        assert_eq!(outcome.stages[0].items, 1);
    }
}
#[test]
fn n12_distinct_counts_never_add_stages_or_attempts() {
    let capture = page(true, ItemDisposition::Accepted);
    let mut discovery = capture.clone();
    discovery.stage = Stage::Discovery;
    discovery.items[0].disposition = ItemDisposition::Discovered;
    let outcome = reconcile(
        &[discovery, capture.clone()],
        &[frontier(&capture, 9)],
        Reason::None,
    )
    .unwrap();
    assert!(outcome.success());
    assert_eq!(outcome.distinct_items, 1);
    assert_eq!(outcome.attempts, 9);
    assert_eq!(outcome.attempted_items, 1);
    assert_eq!(outcome.stages[1].attempted, 1);
    assert_eq!(outcome.stages.len(), 2);
    assert_eq!(outcome.stages[1].accepted, 1);
    assert!(
        reconcile(
            &[capture.clone(), capture.clone()],
            &[frontier(&capture, 9)],
            Reason::None
        )
        .is_err()
    );
}
#[test]
fn n12_every_item_has_capture_or_reason() {
    let mut capture = page(true, ItemDisposition::Accepted);
    capture.items[0].evidence = None;
    assert!(reconcile(&[capture.clone()], &[frontier(&capture, 1)], Reason::None).is_err());
    for disposition in [
        ItemDisposition::Denied,
        ItemDisposition::Blocked,
        ItemDisposition::Refused,
        ItemDisposition::Pending,
        ItemDisposition::Withdrawn,
        ItemDisposition::Unchanged,
    ] {
        let page = page(true, disposition);
        let outcome =
            reconcile(slice::from_ref(&page), &[frontier(&page, 1)], Reason::None).unwrap();
        assert_eq!(outcome.stages[0].items, 1);
    }
}
/// One bounded page with explicit counting unit and evidence.
fn page(complete: bool, disposition: ItemDisposition) -> InventoryPage {
    InventoryPage {
        schema: InventorySchema::V1,
        partition: Handle::new(),
        stage: Stage::Capture,
        complete,
        items: vec![StageItem {
            item: Handle::new(),
            disposition,
            evidence: Some(Handle::new()),
        }],
    }
}

/// Actual N04 frontier row is the sole dispatch-count authority.
fn frontier(page: &InventoryPage, attempts: u64) -> Item {
    Item {
        id: page.items[0].item.to_string().parse().unwrap(),
        source: "docs".into(),
        job: Handle::new().to_string().parse().unwrap(),
        request: NewItem {
            fetch_identity: "https://example.test/docs".into(),
            authorization_context: Digest::of(b"public"),
            representation_profile: Digest::of(b"profile"),
        },
        attempts,
        epoch: attempts,
        capture: match page.items[0].disposition {
            ItemDisposition::Accepted | ItemDisposition::Unchanged => {
                Some(Digest::of(b"verified envelope"))
            }
            _ => None,
        },
    }
}

#[test]
fn n12_capture_acceptance_requires_frontier_acknowledgment() {
    let page = page(true, ItemDisposition::Accepted);
    let mut item = frontier(&page, 1);
    item.capture = None;
    assert_eq!(
        reconcile(slice::from_ref(&page), &[item], Reason::None),
        Err(ReceiptError::Invalid)
    );
}
#[test]
fn n12_denied_discovery_is_not_an_eligible_pending_capture() {
    let mut page = page(true, ItemDisposition::Denied);
    page.stage = Stage::Discovery;
    let outcome = reconcile(&[page], &[], Reason::None).unwrap();
    assert!(outcome.success());
    assert_eq!(outcome.attempted_items, 0);
}
#[test]
fn n12_retry_counts_zero_attempts_and_inventory_bounds_are_exact() {
    let mut page = page(true, ItemDisposition::Pending);
    let item = frontier(&page, 0);
    let outcome = reconcile(slice::from_ref(&page), slice::from_ref(&item), Reason::None).unwrap();
    assert_eq!(outcome.attempted_items, 0);
    assert_eq!(outcome.attempts, 0);
    assert_eq!(outcome.stages[0].attempted, 0);
    assert!(reconcile(slice::from_ref(&page), &[item.clone(), item], Reason::None).is_err());
    page.items = (0..1001)
        .map(|_| StageItem {
            item: Handle::new(),
            disposition: ItemDisposition::Pending,
            evidence: None,
        })
        .collect();
    assert_eq!(
        reconcile(&[page], &[], Reason::None),
        Err(ReceiptError::Invalid)
    );
}
#[test]
fn n12_attempt_counter_overflow_refuses() {
    let pending = page(true, ItemDisposition::Pending);
    let first = frontier(&pending, u64::MAX);
    let other = page(true, ItemDisposition::Pending);
    let second = frontier(&other, 1);
    assert!(reconcile(&[pending, other], &[first, second], Reason::None).is_err());
}

#[test]
fn n12_eligible_discovery_and_non_capture_discovered_remain_pending() {
    let mut discovered = page(true, ItemDisposition::Discovered);
    discovered.stage = Stage::Discovery;
    let missing = discovered.items[0].item;
    let outcome = reconcile(&[discovered], &[], Reason::None).unwrap();
    assert_eq!(outcome.status, Status::Partial);
    assert_eq!(outcome.pending_without_disposition, vec![missing]);
    let capture = page(true, ItemDisposition::Discovered);
    let outcome = reconcile(
        slice::from_ref(&capture),
        &[frontier(&capture, 1)],
        Reason::None,
    )
    .unwrap();
    assert_eq!(outcome.status, Status::Partial);
}

/// Pure inventory counting uses a verifier; storage substitutions use real Database tests.
struct CountingCaptures;
impl Captures for CountingCaptures {
    fn capture_for(&self, _: &Scope, _: &Item) -> Result<Option<Handle>, ReceiptError> {
        Ok(None)
    }

    fn read_capture(
        &self,
        _context: &CaptureContext,
        _capture: Handle,
        _max_bytes: u64,
    ) -> Result<(CaptureEnvelope, Option<Vec<u8>>), ReceiptError> {
        Err(ReceiptError::Invalid)
    }

    fn prepare_capture(
        &self,
        _: &CaptureContext,
        _: &CaptureEnvelope,
        _: &[u8],
        _: u64,
    ) -> Result<PreparedCapture, ReceiptError> {
        Err(ReceiptError::Storage)
    }
    fn acknowledge_capture(&self, _: &CaptureContext, _: Handle) -> Result<(), ReceiptError> {
        Err(ReceiptError::Storage)
    }
    fn check_capture(
        &self,
        _: &CaptureContext,
        _: &CaptureEnvelope,
        _: &[u8],
    ) -> Result<u64, ReceiptError> {
        Err(ReceiptError::Storage)
    }
    fn capture_bytes(&self, _: &CaptureEnvelope) -> Result<u64, ReceiptError> {
        Err(ReceiptError::Storage)
    }
    fn verify_capture(&self, _: &Scope, _: &Item, _: Handle) -> Result<(), ReceiptError> {
        Ok(())
    }
}
/// This file tests only inventory algebra, not kernel artifact verification.
fn reconcile(
    pages: &[InventoryPage],
    frontier: &[Item],
    reason: Reason,
) -> Result<Outcome, ReceiptError> {
    reconcile_verified(
        pages,
        frontier,
        reason,
        (
            &CountingCaptures,
            &"workspace/default/collection/garden".parse().unwrap(),
        ),
    )
}
