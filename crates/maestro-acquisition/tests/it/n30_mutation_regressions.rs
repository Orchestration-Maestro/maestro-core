//! Regression checks for scope inheritance and pre-exposure/notification guards.
use super::{
    n30_support::{Crash, Events, Fixture, Grant},
    support,
};
use maestro_acquisition::{
    Ref,
    adaptation::{AtomicCommit, Change, ConfigurationWriter, LocalWriter, Notify, WriteError},
};
use maestro_kernel::{
    acquisition::{Progress, Receipts},
    scope::{Right, Scope},
};
use std::fs;

#[test]
fn n30_proposal_inherits_private_report_scope() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let private: Scope = "workspace/default/collection/private-report"
        .parse()
        .unwrap();
    fixture
        .db
        .grant(principal.id, &private, Right::Read, "owner")
        .unwrap();
    let mut proposal = fixture.proposal.clone();
    proposal.report = fixture.db.retain(&private, b"private report", &[]).unwrap();
    let writer = LocalWriter::open(
        &fixture.root.join("overlay"),
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let reference = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
        )
        .unwrap();
    fixture
        .db
        .grant(
            "garden-only",
            &fixture
                .manifest
                .resource
                .scope_tags
                .first()
                .unwrap()
                .parse()
                .unwrap(),
            Right::Read,
            "owner",
        )
        .unwrap();
    assert!(
        fixture
            .db
            .read("garden-only", reference.id.parse().unwrap())
            .unwrap()
            .is_none()
    );
}

#[test]
fn n30_activation_inherits_gate_only_scope() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let private: Scope = "workspace/default/collection/private-gate".parse().unwrap();
    fixture
        .db
        .grant(principal.id, &private, Right::Read, "owner")
        .unwrap();
    let gate = fixture.db.retain(&private, b"private gate", &[]).unwrap();
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
    let active = writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            gate,
        )
        .unwrap();
    fixture
        .db
        .grant(
            "garden-only",
            &fixture
                .manifest
                .resource
                .scope_tags
                .first()
                .unwrap()
                .parse()
                .unwrap(),
            Right::Read,
            "owner",
        )
        .unwrap();
    assert!(
        fixture
            .db
            .read("garden-only", active.id.parse().unwrap())
            .unwrap()
            .is_none()
    );
}

#[derive(Debug)]
struct FailedNotification;
impl Notify for FailedNotification {
    fn notify(&self, _: Progress) -> Result<(), WriteError> {
        Err(WriteError::Storage)
    }
}
#[test]
fn n30_failed_notification_recovers_and_retries() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let root = fixture.root.join("overlay");
    let mut context = fixture.context(&principal, &events, &Grant, &AtomicCommit);
    context.notify = &FailedNotification;
    let writer = LocalWriter::open(&root, context, &fixture.manifest).unwrap();
    let proposal = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &fixture.proposal,
        )
        .unwrap();
    assert_eq!(
        writer.activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            fixture.gate
        ),
        Err(WriteError::Storage)
    );
    assert!(root.join("recovery.json").exists());
    drop(writer);
    LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    assert_eq!(events.0.lock().unwrap().len(), 1);
    assert!(!root.join("recovery.json").exists());
    LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    assert_eq!(events.0.lock().unwrap().len(), 1);
}

#[test]
fn n30_invalid_proposal_never_changes_manifest() {
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
    let before = fs::read(root.join("manifest.json")).unwrap();
    // Commit(false) prevents a later recovery decode from masking pre-exposure validation.
    let crash = Crash(false);
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &crash),
        &fixture.manifest,
    )
    .unwrap();
    let mut proposal = fixture.proposal.clone();
    proposal.changes = vec![Change::SetCleanup {
        rules: Ref {
            id: "../invalid".into(),
            digest: proposal.candidate.clone(),
        },
    }];
    assert!(
        writer
            .propose(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &proposal
            )
            .is_err()
    );
    assert_eq!(fs::read(root.join("manifest.json")).unwrap(), before);
    assert!(!root.join("recovery.json").exists());
}
