//! Recovery, rollback and filesystem refusal contracts.
use super::{
    n30_support::{Crash, Events, Fixture, Grant},
    support,
};
use maestro_acquisition::adaptation::{
    AtomicCommit, ConfigurationWriter, HeldAuthority, LocalWriter, WriteError,
};
use maestro_kernel::{
    acquisition::{Handle, Receipts},
    artifact::Digest,
};
use serde_json::{Value, json};
use std::fs;
#[test]
fn n30_recovery_checks_current_authority_before_notification() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let root = fixture.root.join("overlay");
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let reference = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &fixture.proposal,
        )
        .unwrap();
    drop(writer);
    let crash = Crash(true);
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &crash),
        &fixture.manifest,
    )
    .unwrap();
    assert_eq!(
        writer.activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &reference,
            fixture.gate
        ),
        Err(WriteError::Storage)
    );
    drop(writer);
    assert!(
        LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &HeldAuthority, &AtomicCommit),
            &fixture.manifest
        )
        .is_err()
    );
    assert!(events.0.lock().unwrap().is_empty());
    assert!(root.join("recovery.json").exists());
}

#[test]
fn n30_recovery_rejects_unrelated_pointer_or_forged_notification() {
    for field in ["old", "event_handle", "status", "reason"] {
        let fixture = Fixture::new();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let root = fixture.root.join("overlay");
        let writer = LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &fixture.manifest,
        )
        .unwrap();
        let reference = writer
            .propose(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &fixture.proposal,
            )
            .unwrap();
        drop(writer);
        let crash = Crash(field != "old");
        let writer = LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &crash),
            &fixture.manifest,
        )
        .unwrap();
        assert_eq!(
            writer.activate(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &reference,
                fixture.gate
            ),
            Err(WriteError::Storage)
        );
        drop(writer);
        let mut marker: Value =
            serde_json::from_slice(&fs::read(root.join("recovery.json")).unwrap()).unwrap();
        match field {
            "old" => marker["old"] = json!(Digest::of(b"unrelated")),
            "event_handle" => marker["event"]["receipt"] = json!(Handle::new()),
            "status" => marker["event"]["status"] = json!("partial"),
            _ => marker["event"]["reason"] = json!("revoked"),
        }
        fs::write(
            root.join("recovery.json"),
            serde_json::to_vec(&marker).unwrap(),
        )
        .unwrap();
        assert!(
            LocalWriter::open(
                &root,
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &fixture.manifest
            )
            .is_err(),
            "{field}"
        );
        assert!(events.0.lock().unwrap().is_empty());
        assert!(root.join("recovery.json").exists());
    }
}

#[test]
fn n30_rollback_cas_and_current_authority_are_required() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let root = fixture.root.join("overlay");
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let reference = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &fixture.proposal,
        )
        .unwrap();
    let active = writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &reference,
            fixture.gate,
        )
        .unwrap();
    assert_eq!(
        writer.rollback(
            &fixture.manifest.active,
            &fixture.manifest.baseline,
            fixture.gate
        ),
        Err(WriteError::Conflict)
    );
    assert_eq!(
        writer.rollback(&active, &reference, fixture.gate),
        Err(WriteError::Held)
    );
    drop(writer);
    let held = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &HeldAuthority, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    assert_eq!(
        held.rollback(&active, &fixture.manifest.baseline, fixture.gate),
        Err(WriteError::Held)
    );
    drop(held);
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let rollback = writer
        .rollback(&active, &fixture.manifest.baseline, fixture.gate)
        .unwrap();
    assert_ne!(rollback, active);
    assert_eq!(writer.current().unwrap().active, rollback);
    let artifact = fixture
        .db
        .read(principal.id, rollback.id.parse().unwrap())
        .unwrap()
        .unwrap();
    let activation: Value = serde_json::from_slice(artifact.bytes()).unwrap();
    assert_eq!(activation["restores"], json!(fixture.manifest.baseline));
    assert_eq!(events.0.lock().unwrap().len(), 2);
}

#[test]
fn n30_pointer_symlinks_are_not_followed() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let root = fixture.root.join("overlay");
        LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &fixture.manifest,
        )
        .unwrap();
        let path = root.join("manifest.json");
        let trusted = fixture.root.join("trusted-manifest.json");
        fs::rename(&path, &trusted).unwrap();
        let bytes = fs::read(&trusted).unwrap();
        symlink(&trusted, &path).unwrap();
        assert!(
            LocalWriter::open(
                &root,
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &fixture.manifest
            )
            .is_err()
        );
        assert_eq!(fs::read(trusted).unwrap(), bytes);
    }
}

#[test]
fn n30_rollback_stale_existing_active_and_wrong_previous_refuse() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let writer = LocalWriter::open(
        &fixture.root.join("overlay"),
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let proposal = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &fixture.proposal,
        )
        .unwrap();
    let first = writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            fixture.gate,
        )
        .unwrap();
    let mut candidate = fixture.proposal.clone();
    candidate.expected_active = first.clone();
    candidate.rollback = first.clone();
    let proposal = writer
        .propose(&fixture.manifest.baseline, &first, &candidate)
        .unwrap();
    let second = writer
        .activate(&fixture.manifest.baseline, &first, &proposal, fixture.gate)
        .unwrap();
    assert_eq!(
        writer.rollback(&first, &fixture.manifest.baseline, fixture.gate),
        Err(WriteError::Conflict)
    );
    assert_eq!(
        writer.rollback(&second, &fixture.manifest.baseline, fixture.gate),
        Err(WriteError::Held)
    );
    let restored = writer.rollback(&second, &first, fixture.gate).unwrap();
    assert_eq!(writer.current().unwrap().active, restored);
}

#[test]
fn n30_current_reads_hold_revoked_activation_authority() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let root = fixture.root.join("overlay");
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let proposal = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &fixture.proposal,
        )
        .unwrap();
    writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            fixture.gate,
        )
        .unwrap();
    drop(writer);
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &HeldAuthority, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    assert_eq!(writer.current(), Err(WriteError::Held));
}

#[test]
fn n30_unrelated_proposal_recovery_pointer_refuses_without_masking() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let root = fixture.root.join("overlay");
    LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let crash = Crash(false);
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &crash),
        &fixture.manifest,
    )
    .unwrap();
    assert_eq!(
        writer.propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &fixture.proposal
        ),
        Err(WriteError::Storage)
    );
    drop(writer);
    let mut marker: Value =
        serde_json::from_slice(&fs::read(root.join("recovery.json")).unwrap()).unwrap();
    assert!(marker["event"].is_null());
    marker["old"] = json!(Digest::of(b"unrelated pointer"));
    fs::write(
        root.join("recovery.json"),
        serde_json::to_vec(&marker).unwrap(),
    )
    .unwrap();
    assert!(
        LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &fixture.manifest
        )
        .is_err()
    );
    assert!(root.join("recovery.json").exists());
}
