//! Exact initial/prior snapshot rollback through the existing N30 lineage.
use super::{
    n30_support::{Events, Fixture, Grant},
    n57_processing_artifacts::initial,
    n57_support as support,
};
use maestro_acquisition::{
    Ref,
    adaptation::{Activation, AtomicCommit, ConfigurationWriter, LocalWriter, Proposal, storage},
};
use maestro_kernel::{acquisition::Receipts, artifact::Digest};
#[test]
fn n57_prior_snapshot_rollback() {
    let fixture = Fixture::new();
    let principal = super::support::principal(&fixture.scopes);
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
    let active = writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            fixture.gate,
        )
        .unwrap();
    let mut next = fixture.proposal.clone();
    next.expected_active = active.clone();
    next.rollback = active.clone();
    let proposal = writer
        .propose(&fixture.manifest.baseline, &active, &next)
        .unwrap();
    let second = writer
        .activate(&fixture.manifest.baseline, &active, &proposal, fixture.gate)
        .unwrap();
    let rolled = writer.rollback(&second, &active, fixture.gate).unwrap();
    let reader = support::reader(
        &fixture.db,
        &fixture.collection,
        &fixture.catalog,
        &principal,
    );
    let (_, restored) = reader.current(&writer.current().unwrap()).unwrap();
    let (_, expected) = reader.candidate(&fixture.proposal).unwrap();
    assert_eq!(restored.snapshot, expected.snapshot);
    assert!(writer.rollback(&rolled, &rolled, fixture.gate).is_err());
}
#[test]
fn n57_baseline_snapshot_rollback() {
    let events = Events::default();
    // A fresh overlay isolates baseline restoration from the prior-target exercise.
    let fixture = Fixture::new();
    let principal = super::support::principal(&fixture.scopes);
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
            fixture.gate,
        )
        .unwrap();
    let rollback = writer.rollback(&active, &fixture.manifest.baseline, fixture.gate);
    assert!(rollback.is_ok());
    rollback.unwrap();
    let manifest = writer.current().unwrap();
    let reader = support::reader(
        &fixture.db,
        &fixture.collection,
        &fixture.catalog,
        &principal,
    );
    let (pin, restored) = reader.current(&manifest).unwrap();
    assert_eq!(pin, fixture.manifest.processing_baseline);
    assert_eq!(restored.snapshot, initial(&fixture));
    assert_ne!(pin.digest, manifest.baseline.digest);
    let activation: serde_json::Value = serde_json::from_slice(
        fixture
            .db
            .read("synthetic-reader", manifest.active.id.parse().unwrap())
            .unwrap()
            .unwrap()
            .bytes(),
    )
    .unwrap();
    let proposal_ref: Ref = serde_json::from_value(activation["proposal"].clone()).unwrap();
    let rollback: Proposal =
        storage::artifact(&fixture.db, "synthetic-reader", &proposal_ref).unwrap();
    assert_eq!(
        rollback.candidate,
        fixture.manifest.processing_baseline.digest
    );
    assert!(
        rollback
            .evidence
            .contains(&fixture.manifest.processing_baseline.id.parse().unwrap())
    );
    let notifications = serde_json::to_vec(&*events.0.lock().unwrap()).unwrap();
    assert!(!String::from_utf8(notifications).unwrap().contains("canary"));
}

#[test]
fn n57_current_manifest_baseline_must_match_verified_snapshot() {
    let fixture = Fixture::new();
    let principal = super::support::principal(&fixture.scopes);
    let reader = support::reader(
        &fixture.db,
        &fixture.collection,
        &fixture.catalog,
        &principal,
    );
    assert!(reader.current(&fixture.manifest).is_ok());
    let mut manifest = fixture.manifest.clone();
    manifest.baseline.digest = Digest::of(b"different baseline");
    manifest.active = manifest.baseline.clone();
    assert!(reader.current(&manifest).is_err());
}

#[test]
fn n57_rollback_candidate_must_name_exact_restore_snapshot() {
    let fixture = Fixture::new();
    let principal = super::support::principal(&fixture.scopes);
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
    let active = writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            fixture.gate,
        )
        .unwrap();
    writer
        .rollback(&active, &fixture.manifest.baseline, fixture.gate)
        .unwrap();
    let mut manifest = writer.current().unwrap();
    let reader = support::reader(
        &fixture.db,
        &fixture.collection,
        &fixture.catalog,
        &principal,
    );
    assert!(reader.current(&manifest).is_ok());
    let mut activation: Activation =
        storage::artifact(&fixture.db, "synthetic-reader", &manifest.active).unwrap();
    let mut rollback: Proposal =
        storage::artifact(&fixture.db, "synthetic-reader", &activation.proposal).unwrap();
    rollback.candidate = fixture.proposal.candidate.clone();
    rollback.evidence = vec![fixture.gate, *fixture.proposal.evidence.last().unwrap()];
    let forged = support::retain(&fixture.db, &rollback, &rollback.evidence);
    *manifest.proposals.last_mut().unwrap() = forged.clone();
    activation.proposal = forged;
    let forged = support::retain(
        &fixture.db,
        &activation,
        &[activation.proposal.id.parse().unwrap(), fixture.gate],
    );
    *manifest.activations.last_mut().unwrap() = forged.clone();
    manifest.active = forged;
    assert!(reader.current(&manifest).is_err());
}
