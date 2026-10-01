//! Real persistence interrupted at the consumer boundary, without a second store.
use super::{n11_support, n12_support::Fixture};
use maestro_acquisition::{
    capture::{CaptureBudget, CaptureContext, CaptureEnvelope, Captures, prepare},
    transport::budget::Usage,
};
use maestro_kernel::{
    acquisition::{DispatchRequest, Frontier, Handle, LeaseRequest, PreparedCapture, ReceiptError},
    store::Database,
};
use std::time::Duration;

/// Fault adapter models a caller dying after the durable prepare commit.
struct CrashAfterPrepare<'a>(&'a Database);
impl Captures for CrashAfterPrepare<'_> {
    fn prepare_capture(
        &self,
        context: &CaptureContext,
        envelope: &CaptureEnvelope,
        bytes: &[u8],
    ) -> Result<PreparedCapture, ReceiptError> {
        self.0.prepare_capture(context, envelope, bytes)?;
        Err(ReceiptError::Storage)
    }
    fn acknowledge_capture(
        &self,
        _context: &CaptureContext,
        _capture: Handle,
    ) -> Result<(), ReceiptError> {
        Err(ReceiptError::Storage)
    }
}
#[test]
fn n12_caller_crash_after_commit_replays_without_knowing_capture_handle() {
    let fixture = Fixture::new();
    let (_, resources) = n11_support::resources();
    let mut reservation = n11_support::reserve(&resources, Usage::default());
    let bounds = [n11_support::limits()];
    let mut budget = CaptureBudget {
        reservation: &mut reservation,
        bounds: &bounds,
        usage: Usage::default(),
    };
    assert_eq!(
        prepare(
            &CrashAfterPrepare(&fixture.db),
            &fixture.policy,
            &fixture.context,
            (&fixture.envelope, b"body"),
            &mut budget
        ),
        Err(ReceiptError::Storage)
    );
    let pending = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        10,
    )
    .unwrap();
    assert!(pending.first().unwrap().capture.is_none());
    let replay = fixture.prepare().unwrap();
    assert_eq!(
        fixture.db.acknowledge_capture(&fixture.context, replay),
        Ok(())
    );
    assert_eq!(
        fixture.db.acknowledge_capture(&fixture.context, replay),
        Ok(())
    );
    let accepted = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        10,
    )
    .unwrap();
    assert_eq!(accepted.len(), 1);
    assert!(accepted.first().unwrap().capture.is_some());
}
#[test]
fn n12_lease_loss_then_current_epoch_reuses_prepared_capture() {
    let mut fixture = Fixture::new();
    let capture = fixture.prepare().unwrap();
    fixture.context.now += Duration::from_secs(31);
    fixture.context.writer = fixture
        .db
        .lease_source(
            "notes",
            &"workspace/default/collection/garden".parse().unwrap(),
            LeaseRequest {
                holder: "replacement",
                now: fixture.context.now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    assert_eq!(
        fixture.db.acknowledge_capture(&fixture.context, capture),
        Err(ReceiptError::Conflict)
    );
    fixture.context.item = fixture
        .db
        .lease(
            &fixture.context.writer,
            fixture.context.item.item,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "replacement",
                    now: fixture.context.now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 3,
            },
        )
        .unwrap();
    assert_eq!(fixture.prepare(), Ok(capture));
    assert_eq!(
        fixture.db.acknowledge_capture(&fixture.context, capture),
        Ok(())
    );
    let accepted = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        10,
    )
    .unwrap();
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted.first().unwrap().attempts, 2);
    assert!(accepted.first().unwrap().capture.is_some());
}
