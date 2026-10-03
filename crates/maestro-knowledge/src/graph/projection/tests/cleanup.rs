//! Cleanup policy: authorization first, immutable receipts retained, no engine required.
#[cfg(unix)]
use super::super::cleanup::CleanupOutcome;
use super::super::cleanup::{Cleanup, CleanupError};
use super::cleanup_support::{Fixture, timing};
use maestro_canonicalization::{ControlFile, FileLock, LockMode, OwnedRoot, SystemFileLock};
use maestro_kernel::{job::JobState, scope::Right};
use serde_json::json;
use std::{fs, io};

fn prepare(fixture: &Fixture, apply: bool) -> Result<Cleanup, CleanupError> {
    Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &fixture.path.join("graph"),
        fixture.receipt.generation_id,
        apply,
    )
}

#[test]
fn cleanup_heartbeat_refusal_preserves_file() {
    use maestro_kernel::job::LeaseTiming;
    use std::time::{Duration, UNIX_EPOCH};
    let fixture = Fixture::new();
    fixture.retire();
    let cleanup = prepare(&fixture, true).unwrap();
    let mut lease = fixture.lease();
    // The exact live job passes validation; only the renewal timestamp is invalid.
    let invalid = LeaseTiming {
        now: UNIX_EPOCH - Duration::from_secs(1),
        ..timing()
    };
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, invalid)
            .unwrap_err(),
        CleanupError::LeaseInvalid
    );
    assert!(
        fixture
            .path
            .join("graph")
            .join(&fixture.receipt.file_name)
            .exists()
    );
    #[cfg(unix)]
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap(),
        CleanupOutcome::Removed
    );
    #[cfg(windows)]
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::Unsupported
    );
}

#[test]
fn cleanup_unsafe_root_has_fixed_classification() {
    let fixture = Fixture::new();
    fixture.retire();
    let path = fixture.path.join("graph/.access.guard");
    fs::hard_link(&path, fixture.path.join("guard-alias")).unwrap();
    for apply in [false, true] {
        let error = prepare(&fixture, apply).unwrap_err();
        assert_eq!(error, CleanupError::UnsafeRoot);
        assert_eq!(error.reason(), "unsafe_root");
    }
    assert!(
        fixture
            .path
            .join("graph")
            .join(&fixture.receipt.file_name)
            .exists()
    );
    fs::remove_file(fixture.path.join("guard-alias")).unwrap();
    assert!(prepare(&fixture, false).unwrap().present());
}

#[test]
fn cleanup_refuses_every_retained_state_with_retired_and_failed_neighbours() {
    let fixture = Fixture::new();
    assert_eq!(
        prepare(&fixture, false).unwrap_err(),
        CleanupError::Ineligible
    );
    fixture
        .database
        .publish_generation(fixture.receipt.generation_id)
        .unwrap();
    assert_eq!(
        prepare(&fixture, true).unwrap_err(),
        CleanupError::Ineligible
    );
    fixture
        .database
        .retire_generation(fixture.receipt.generation_id)
        .unwrap();
    let preview = prepare(&fixture, false).unwrap();
    assert_eq!(preview.receipt(), &fixture.receipt);
    assert!(preview.present());
    drop(preview);
    let failed = Fixture::new();
    failed
        .database
        .fail_generation(failed.receipt.generation_id)
        .unwrap();
    assert!(prepare(&failed, false).unwrap().present());
    assert!(
        failed
            .path
            .join("graph")
            .join(&failed.receipt.file_name)
            .exists()
    );
}

#[test]
fn cleanup_unknown_and_unauthorized_are_identical_and_missing_guard_precedes_lookup() {
    let fixture = Fixture::new();
    fixture.retire();
    let path = fixture.path.join("graph");
    let unknown = Cleanup::prepare(&fixture.database, "cleaner", &path, 9000, false).unwrap_err();
    let denied = Cleanup::prepare(
        &fixture.database,
        "denied",
        &path,
        fixture.receipt.generation_id,
        false,
    )
    .unwrap_err();
    assert_eq!(unknown, CleanupError::TargetUnavailable);
    assert_eq!(unknown, denied);
    fs::remove_file(path.join(".access.guard")).unwrap();
    assert_eq!(
        prepare(&fixture, false).unwrap_err(),
        CleanupError::GuardMissing
    );
    assert_eq!(
        Cleanup::prepare(&fixture.database, "cleaner", &path, 9000, true).unwrap_err(),
        CleanupError::GuardMissing
    );
    assert!(!path.join(".access.guard").exists());
}

#[test]
fn cleanup_preview_cannot_apply_and_exclusive_guard_blocks_open() {
    let fixture = Fixture::new();
    fixture.retire();
    let preview = prepare(&fixture, false).unwrap();
    let mut lease = fixture.lease();
    assert_eq!(
        preview
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::LeaseInvalid
    );
    assert_eq!(prepare(&fixture, true).unwrap_err(), CleanupError::Busy);
    drop(preview);
    let cleanup = prepare(&fixture, true).unwrap();
    assert_eq!(prepare(&fixture, false).unwrap_err(), CleanupError::Busy);
    drop(cleanup);
    assert!(prepare(&fixture, false).is_ok());
}

#[test]
fn cleanup_unsupported_lock_fails_closed_before_selection() {
    struct Unsupported;
    impl FileLock for Unsupported {
        fn acquire(&self, _file: &fs::File, _mode: LockMode, _wait: bool) -> io::Result<()> {
            Err(io::ErrorKind::Unsupported.into())
        }
    }
    let fixture = Fixture::new();
    fixture.retire();
    for apply in [false, true] {
        assert_eq!(
            super::super::cleanup::access_guard(&fixture.path.join("graph"), apply, &Unsupported)
                .unwrap_err(),
            CleanupError::Unsupported
        );
    }
    assert!(
        fixture
            .path
            .join("graph")
            .join(&fixture.receipt.file_name)
            .exists()
    );
}

#[test]
fn cleanup_apply_rechecks_authority_and_scoped_lease() {
    let fixture = Fixture::new();
    fixture.retire();
    let cleanup = prepare(&fixture, true).unwrap();
    let mut lease = fixture.lease();
    let scope = "workspace/default/collection/cleanup".parse().unwrap();
    fixture
        .database
        .revoke("cleaner", &scope, Right::Read, "test")
        .unwrap();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::TargetUnavailable
    );
    fixture
        .database
        .grant("cleaner", &scope, Right::Read, "test")
        .unwrap();
    fixture
        .database
        .complete_job(&lease, JobState::Cancelled, &json!({}))
        .unwrap();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap_err(),
        CleanupError::LeaseInvalid
    );
    assert!(
        fixture
            .path
            .join("graph")
            .join(&fixture.receipt.file_name)
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn cleanup_apply_keeps_authority_unrelated_bytes_and_retries_after_unlink() {
    let fixture = Fixture::new();
    fixture.retire();
    let graph = fixture.path.join("graph");
    let attachment = fixture
        .database
        .graph_attachment(&fixture.scopes, fixture.receipt.generation_id)
        .unwrap();
    let generation = fixture
        .database
        .generation(&fixture.scopes, fixture.receipt.generation_id)
        .unwrap();
    let cleanup = prepare(&fixture, true).unwrap();
    let mut lease = fixture.lease();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap(),
        CleanupOutcome::Removed
    );
    // Simulates death after unlink and before the cleanup job outcome is committed.
    drop(cleanup);
    let retry = prepare(&fixture, true).unwrap();
    assert!(!retry.present());
    assert_eq!(
        retry
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap(),
        CleanupOutcome::AlreadyMissing
    );
    assert_eq!(
        fixture
            .database
            .projection_ready(&fixture.scopes, fixture.receipt.generation_id)
            .unwrap(),
        Some(fixture.receipt.clone())
    );
    assert_eq!(
        fixture
            .database
            .graph_attachment(&fixture.scopes, fixture.receipt.generation_id)
            .unwrap(),
        attachment
    );
    assert_eq!(
        fixture
            .database
            .generation(&fixture.scopes, fixture.receipt.generation_id)
            .unwrap(),
        generation
    );
    assert_eq!(
        fs::read(graph.join("orphan.lbdb")).unwrap(),
        b"preserve orphan exactly"
    );
    assert!(graph.join(".access.guard").exists());
    assert!(graph.join(".writer.guard").exists());
}

#[test]
fn cleanup_busy_guard_precedes_unknown_target_and_receipt_lookup() {
    let fixture = Fixture::new();
    fixture.retire();
    let root = OwnedRoot::open(&fixture.path.join("graph"), false).unwrap();
    let reader = root.open_control(ControlFile::Access).unwrap();
    reader
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
    assert_eq!(
        Cleanup::prepare(
            &fixture.database,
            "cleaner",
            &fixture.path.join("graph"),
            9000,
            true
        )
        .unwrap_err(),
        CleanupError::Busy
    );
    assert!(prepare(&fixture, false).is_ok());
    drop(reader);
    assert!(prepare(&fixture, true).is_ok());
}

#[test]
fn cleanup_fixed_reason_codes_and_diagnostics_are_exact() {
    use super::super::cleanup::CleanupOutcome;
    for (error, reason, diagnostic) in [
        (
            CleanupError::TargetUnavailable,
            "target_unavailable",
            "unknown or unauthorized graph cleanup target",
        ),
        (
            CleanupError::Ineligible,
            "generation_retained",
            "graph generation is retained; only Retired or Failed with a receipt can be cleaned",
        ),
        (
            CleanupError::ReceiptMissing,
            "receipt_missing",
            "graph generation has no readiness receipt; preserve receiptless files for recovery",
        ),
        (
            CleanupError::GuardMissing,
            "guard_missing",
            "graph access guard is missing; run maestro setup --yes",
        ),
        (
            CleanupError::Busy,
            "access_busy",
            "graph access is busy; let readers or writers finish, then retry cleanup",
        ),
        (
            CleanupError::Unsupported,
            "unsupported",
            "platform cannot safely lock or durably remove this graph file; keep the file",
        ),
        (
            CleanupError::UnsafeRoot,
            "unsafe_root",
            "graph root or access guard is unsafe; restore the owned private root \
          and guards before cleanup",
        ),
        (
            CleanupError::UnsafeFile,
            "unsafe_file",
            "graph receipt file is unsafe or replaced; preserve the file \
             and report it for recovery",
        ),
        (
            CleanupError::AuthorityUnavailable,
            "authority_unavailable",
            "graph cleanup authority is unavailable; check the kernel before cleanup",
        ),
        (
            CleanupError::LeaseInvalid,
            "lease_invalid",
            "graph cleanup lease is invalid or lost; retry cleanup",
        ),
    ] {
        assert_eq!(error.reason(), reason);
        assert_eq!(error.to_string(), diagnostic);
    }
    assert_eq!(CleanupOutcome::Removed.reason(), "removed");
    assert_eq!(CleanupOutcome::AlreadyMissing.reason(), "already_missing");
}
