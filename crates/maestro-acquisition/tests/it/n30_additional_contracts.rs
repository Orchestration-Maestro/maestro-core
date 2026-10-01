//! Typed proposal inputs, fresh CAS and scoped inheritance regressions.
use super::{
    n30_support::{Crash, Events, Fixture, Grant},
    support,
};
use maestro_acquisition::{
    AdmissionStatus, Refusal,
    adaptation::{AtomicCommit, Change, ConfigurationWriter, LocalWriter, Proposal, WriteError},
    policy::resource::Visibility,
    validate,
};
use maestro_kernel::{
    acquisition::Receipts,
    artifact::Digest,
    scope::{Right, Scope},
};
use serde_json::{Value, json};
use std::fs;

#[test]
fn n30_activation_expected_active_cas_with_fresh_candidate() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let writer = LocalWriter::open(
        &fixture.root.join("overlay"),
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
    let active = writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &first,
            fixture.gate,
        )
        .unwrap();
    let mut proposal = fixture.proposal.clone();
    proposal.expected_active = active.clone();
    proposal.rollback = active.clone();
    let second = writer
        .propose(&fixture.manifest.baseline, &active, &proposal)
        .unwrap();
    assert_eq!(
        writer.activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &second,
            fixture.gate
        ),
        Err(WriteError::Conflict)
    );
}

#[test]
fn n30_initial_manifest_metadata_cannot_override_baseline() {
    for field in [
        "baseline",
        "collection_id",
        "visibility",
        "scope_tags",
        "owner_ref",
    ] {
        let fixture = Fixture::new();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let mut initial = fixture.manifest.clone();
        match field {
            "baseline" => {
                initial.baseline.digest = Digest::of(b"wrong baseline");
                initial.active = initial.baseline.clone();
            }
            "collection_id" => initial.resource.collection_id = "other".into(),
            "visibility" => initial.resource.visibility = Visibility::Private,
            "scope_tags" => {
                initial.resource.scope_tags = vec!["workspace/default/collection/other".into()];
            }
            _ => initial.resource.owner_ref.id = "other".into(),
        }
        assert!(
            LocalWriter::open(
                &fixture.root.join("overlay"),
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &initial
            )
            .is_err(),
            "{field}"
        );
        assert!(!fixture.root.join("overlay/manifest.json").exists());
    }
}

#[test]
fn n30_closed_change_wire_and_id_shapes_are_enforced() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let writer = LocalWriter::open(
        &fixture.root.join("overlay"),
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let mut value = serde_json::to_value(&fixture.proposal).unwrap();
    value["changes"] = json!([{ "kind": "set_budget", "rules": fixture.manifest.baseline }]);
    assert!(serde_json::from_value::<Proposal>(value).is_err());
    let mut proposal = fixture.proposal.clone();
    proposal.changes = vec![Change::SelectProfile {
        source_id: "../source".into(),
        profile: fixture.manifest.baseline.clone(),
    }];
    assert_eq!(
        writer.propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal
        ),
        Err(WriteError::Refused(Refusal::Invalid))
    );
}

#[test]
fn n30_history_activation_artifacts_are_verified() {
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
    let active = writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &first,
            fixture.gate,
        )
        .unwrap();
    let mut proposal = fixture.proposal.clone();
    proposal.expected_active = active.clone();
    proposal.rollback = active.clone();
    let second = writer
        .propose(&fixture.manifest.baseline, &active, &proposal)
        .unwrap();
    writer
        .activate(&fixture.manifest.baseline, &active, &second, fixture.gate)
        .unwrap();
    let mut manifest = writer.current().unwrap();
    manifest.activations.first_mut().unwrap().digest = Digest::of(b"corrupt history");
    let checked = validate(&fixture.catalog, &fixture.collection, &principal).unwrap();
    let canonical = (checked.policy(), &manifest.activations);
    manifest.effective_digest = Digest::of(&serde_json::to_vec(&canonical).unwrap());
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    // The child's previous identity no longer matches the ordered history.
    assert_eq!(writer.current(), Err(WriteError::Refused(Refusal::Invalid)));
}

#[test]
fn n30_committed_recovery_revalidates_revoked_baseline() {
    let mut fixture = Fixture::new();
    let events = Events::default();
    let root = fixture.root.join("overlay");
    {
        let principal = support::principal(&fixture.scopes);
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
                &proposal,
                fixture.gate
            ),
            Err(WriteError::Storage)
        );
    }
    fixture
        .catalog
        .0
        .get_mut("policy")
        .unwrap()
        .admission
        .status = AdmissionStatus::Revoked;
    let principal = support::principal(&fixture.scopes);
    assert!(
        LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &fixture.manifest
        )
        .is_err()
    );
    assert!(events.0.lock().unwrap().is_empty());
    assert!(root.join("recovery.json").exists());
}

#[test]
fn n30_multiple_inherited_scopes_protect_proposals_and_events() {
    let mut fixture = Fixture::new();
    let secondary: Scope = "workspace/default/collection/secondary".parse().unwrap();
    fixture
        .db
        .grant("synthetic-reader", &secondary, Right::Read, "owner")
        .unwrap();
    let mut policy = support::value(&fixture.catalog, "policy");
    policy["scope_tags"] = json!(["workspace/default/collection/garden", secondary.as_str()]);
    let reference = support::put(&mut fixture.catalog, "policy", &policy);
    fixture.collection.source_policy = Some(reference.clone());
    fixture
        .manifest
        .resource
        .scope_tags
        .push(secondary.to_string());
    fixture.manifest.baseline = reference.clone();
    fixture.manifest.active = reference.clone();
    fixture.proposal.expected_baseline = reference.clone();
    fixture.proposal.expected_active = reference.clone();
    fixture.proposal.rollback = reference;
    fixture.scopes = fixture.db.visible("synthetic-reader").unwrap();
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
    let active = writer
        .activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            fixture.gate,
        )
        .unwrap();
    fixture
        .db
        .revoke(principal.id, &secondary, Right::Read, "owner")
        .unwrap();
    assert!(
        fixture
            .db
            .read(principal.id, proposal.id.parse().unwrap())
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .db
            .read(principal.id, active.id.parse().unwrap())
            .unwrap()
            .is_none()
    );
    let payload = serde_json::to_value(events.0.lock().unwrap().first().unwrap()).unwrap();
    assert_eq!(payload.as_object().unwrap().len(), 3);
    assert!(payload.get("report").is_none());
}

#[test]
fn n30_whitespace_changes_do_not_change_effective_digest() {
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
    let before = writer.current().unwrap();
    let state: Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();
    assert_eq!(writer.current().unwrap(), before);
}
