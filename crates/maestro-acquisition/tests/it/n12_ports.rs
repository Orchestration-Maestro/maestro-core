//! Real persistence interrupted at the consumer boundary, without a second store.
use super::{n11_support, n12_support::Fixture};
use maestro_acquisition::{
    capture::{CaptureBudget, CaptureContext, CaptureEnvelope, Captures, prepare},
    transport::budget::Usage,
};
use maestro_kernel::{acquisition::Item, scope::Scope};
use maestro_kernel::{
    acquisition::{DispatchRequest, Frontier, Handle, LeaseRequest, PreparedCapture, ReceiptError},
    store::Database,
};
use std::time::Duration;

/// Fault adapter models a caller dying after the durable prepare commit.
struct CrashAfterPrepare<'a>(&'a Database, bool);
impl Captures for CrashAfterPrepare<'_> {
    fn capture_for(&self, scope: &Scope, item: &Item) -> Result<Option<Handle>, ReceiptError> {
        self.0.capture_for(scope, item)
    }

    fn read_capture(
        &self,
        context: &CaptureContext,
        capture: Handle,
        max_bytes: u64,
    ) -> Result<(CaptureEnvelope, Option<Vec<u8>>), ReceiptError> {
        self.0.read_capture(context, capture, max_bytes)
    }

    fn prepare_capture(
        &self,
        context: &CaptureContext,
        envelope: &CaptureEnvelope,
        bytes: &[u8],
        max_new_bytes: u64,
    ) -> Result<PreparedCapture, ReceiptError> {
        self.0
            .prepare_capture(context, envelope, bytes, max_new_bytes)?;
        Err(ReceiptError::Storage)
    }
    fn check_capture(
        &self,
        context: &CaptureContext,
        envelope: &CaptureEnvelope,
        bytes: &[u8],
    ) -> Result<u64, ReceiptError> {
        self.0.check_capture(context, envelope, bytes)
    }
    fn capture_bytes(&self, envelope: &CaptureEnvelope) -> Result<u64, ReceiptError> {
        if self.1 && self.0.artifact(&envelope.artifact)?.is_some() {
            return Err(ReceiptError::Storage);
        }
        self.0.capture_bytes(envelope)
    }
    fn verify_capture(
        &self,
        scope: &Scope,
        item: &Item,
        evidence: Handle,
    ) -> Result<(), ReceiptError> {
        self.0.verify_capture(scope, item, evidence)
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
    crash_after_prepare(false);
}
#[test]
fn n12_failed_readback_keeps_full_staging_reservation() {
    crash_after_prepare(true);
}
/// Durable write followed by an unreadable result retains its admitted allocation.
fn crash_after_prepare(unreadable: bool) {
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
            &CrashAfterPrepare(&fixture.db, unreadable),
            &fixture.policy,
            &fixture.context,
            (&fixture.envelope, b"body"),
            &mut budget
        ),
        Err(ReceiptError::Storage)
    );
    assert_eq!(budget.usage.staging_bytes, 824);
    assert_eq!(resources.usage().unwrap().staging_bytes, 824);
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

#[test]
fn n12_fixture_closes_database_before_removing_directory() {
    let fixture = Fixture::new();
    let root = fixture.root.to_path_buf();
    drop(fixture);
    assert!(!root.exists());
}
#[test]
fn n12_reuse_preflight_cannot_write_after_artifact_removal() {
    let fixture = Fixture::new();
    // Preexisting payloads yield zero growth even without a capture link.
    fixture.db.put(b"body", "text/markdown").unwrap();
    fixture
        .db
        .put(
            &serde_json::to_vec(&fixture.envelope).unwrap(),
            "application/json",
        )
        .unwrap();
    let admitted = fixture
        .db
        .check_capture(&fixture.context, &fixture.envelope, b"body")
        .unwrap();
    assert_eq!(admitted, 0);
    fixture.db.collect_garbage().unwrap();
    assert_eq!(
        fixture
            .db
            .prepare_capture(&fixture.context, &fixture.envelope, b"body", admitted),
        Err(ReceiptError::Conflict)
    );
    assert_eq!(fixture.db.check_artifacts().unwrap().recorded, 1);
    assert!(
        fixture
            .db
            .artifact(&fixture.envelope.artifact)
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .db
            .prepare_capture(&fixture.context, &fixture.envelope, b"body", 824)
            .is_ok()
    );
}
