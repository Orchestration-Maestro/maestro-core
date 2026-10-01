//! N30 review regressions: current authority, effective lineage and recovery.
use super::{
    n30_support::{Crash, Events, Fixture, Grant},
    support,
};
use maestro_acquisition::Refusal;
use maestro_acquisition::adaptation::{
    AtomicCommit, ConfigurationWriter, HeldAuthority, LocalWriter, WriteError,
};
use maestro_kernel::{
    acquisition::Receipts,
    scope::{Right, Scope},
};
use serde_json::Value;
use std::fs;

#[test]
fn n30_review_unactivated_revocation_does_not_hold_baseline() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let writer = LocalWriter::open(
        &fixture.root.join("overlay"),
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let private: Scope = "workspace/default/collection/review-private"
        .parse()
        .unwrap();
    fixture
        .db
        .grant(principal.id, &private, Right::Read, "owner")
        .unwrap();
    let evidence = fixture
        .db
        .retain(&private, b"synthetic review private evidence", &[])
        .unwrap();
    let report = fixture
        .db
        .retain(&private, b"synthetic review private report", &[evidence])
        .unwrap();
    let mut proposal = fixture.proposal.clone();
    proposal.evidence = vec![evidence];
    proposal.report = report;
    writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
        )
        .unwrap();
    fixture
        .db
        .revoke(principal.id, &private, Right::Read, "owner")
        .unwrap();
    let current = writer.current();
    println!("current_result={current:?}");
    assert!(
        current.is_ok(),
        "unactivated private history must not block baseline"
    );
    assert_eq!(current.unwrap().active, fixture.manifest.baseline);
}

#[test]
fn n30_review_rollback_after_current_evidence_revocation() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let writer = LocalWriter::open(
        &fixture.root.join("overlay"),
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let private: Scope = "workspace/default/collection/review-private"
        .parse()
        .unwrap();
    fixture
        .db
        .grant(principal.id, &private, Right::Read, "owner")
        .unwrap();
    let evidence = fixture
        .db
        .retain(&private, b"synthetic review private evidence", &[])
        .unwrap();
    let report = fixture
        .db
        .retain(&private, b"synthetic review private report", &[evidence])
        .unwrap();
    let mut proposal = fixture.proposal.clone();
    proposal.evidence = vec![evidence];
    proposal.report = report;
    let reference = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
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
    fixture
        .db
        .revoke(principal.id, &private, Right::Read, "owner")
        .unwrap();
    assert!(
        fixture
            .db
            .read(principal.id, fixture.gate)
            .unwrap()
            .is_some()
    );
    assert!(
        writer.current().is_err(),
        "effective private lineage must refuse"
    );
    let rollback = writer.rollback(&active, &fixture.manifest.baseline, fixture.gate);
    let current = writer.current();
    println!("rollback_result={rollback:?}");
    println!("current_result={current:?}");
    assert!(
        rollback.is_ok(),
        "rollback must not read revoked active payload"
    );
    let rollback = rollback.unwrap();
    let activation = protected_json(&fixture, principal.id, &rollback.id);
    let rollback_proposal = protected_json(
        &fixture,
        principal.id,
        activation["proposal"]["id"].as_str().unwrap(),
    );
    assert_ne!(rollback_proposal["report"], serde_json::json!(report));
    let binding = protected_json(
        &fixture,
        principal.id,
        rollback_proposal["report"].as_str().unwrap(),
    );
    assert_eq!(
        binding,
        serde_json::json!([fixture.gate, active, fixture.manifest.baseline])
    );
    assert!(current.is_ok());
}

fn protected_json(fixture: &Fixture, principal: &str, handle: &str) -> Value {
    let artifact = fixture
        .db
        .read(principal, handle.parse().unwrap())
        .unwrap()
        .unwrap();
    serde_json::from_slice(artifact.bytes()).unwrap()
}

#[test]
fn n30_review_committed_recovery_missing_event_refuses() {
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
    let marker_path = root.join("recovery.json");
    let mut marker: Value = serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    marker["event"] = Value::Null;
    fs::write(&marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();
    drop(writer);
    let result = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &HeldAuthority, &AtomicCommit),
        &fixture.manifest,
    );
    let marker_present = marker_path.exists();
    let notifications = events.0.lock().unwrap().len();
    println!(
        "open_result={result:?} marker_present={marker_present} notifications={notifications}"
    );
    assert!(result.is_err());
    assert!(marker_present);
    assert_eq!(notifications, 0);
}

#[test]
fn n30_review_truncated_activation_chain_refuses() {
    use maestro_acquisition::{adaptation::Change, validate};
    use maestro_kernel::artifact::Digest;
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
    let first = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &fixture.proposal,
        )
        .unwrap();
    let a1 = writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &first,
            fixture.gate,
        )
        .unwrap();
    let mut proposal = fixture.proposal.clone();
    proposal.expected_active = a1.clone();
    proposal.rollback = a1.clone();
    proposal.changes = vec![Change::SetDedupKeys {
        keys: fixture.catalog.0["evidence"].reference.clone(),
    }];
    let second = writer
        .propose(&fixture.manifest.baseline, &a1, &proposal)
        .unwrap();
    let a2 = writer
        .activate(&fixture.manifest.baseline, &a1, &second, fixture.gate)
        .unwrap();
    let mut manifest = writer.current().unwrap();
    assert_eq!(manifest.active, a2);
    manifest.activations.retain(|reference| reference != &a1);
    let checked = validate(&fixture.catalog, &fixture.collection, &principal).unwrap();
    let canonical = (checked.policy(), &manifest.activations);
    manifest.effective_digest = Digest::of(&serde_json::to_vec(&canonical).unwrap());
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let current = writer.current();
    println!("current_result={current:?}");
    assert_eq!(current, Err(WriteError::Refused(Refusal::Invalid)));
}

#[test]
fn n30_review_baseline_read_rechecks_current_collection_grant() {
    let mut refused = Vec::new();
    for direct in [false, true] {
        let fixture = Fixture::new();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let files = fixture.direct();
        let source: &dyn maestro_acquisition::ResourceSource =
            if direct { &files } else { &fixture.catalog };
        let writer = LocalWriter::open(
            &fixture.root.join("overlay"),
            fixture.context_source(&principal, &events, (&Grant, &AtomicCommit), source),
            &fixture.manifest,
        )
        .unwrap();
        let garden: Scope = "workspace/default/collection/garden".parse().unwrap();
        fixture
            .db
            .revoke(principal.id, &garden, Right::Read, "owner")
            .unwrap();
        let current = writer.current();
        println!(
            "source={} current_result={current:?}",
            if direct { "DirectFiles" } else { "synthetic" }
        );
        refused.push(current.is_err());
    }
    assert!(refused.into_iter().all(|value| value));
}
