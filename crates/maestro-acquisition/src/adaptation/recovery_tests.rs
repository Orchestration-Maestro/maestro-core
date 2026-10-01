//! Each transition guard is checked directly, without lineage checks masking it.
use super::{manifest::Activation, recovery::Recovery, storage};
use crate::{
    Ref,
    policy::manifest::{AcquisitionManifest, BaselineKind},
};
use maestro_kernel::{
    acquisition::{Handle, Progress, Reason, Receipts, Status},
    artifact::Digest,
    scope::Right,
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use std::fs;

#[test]
fn n30_proposal_recovery_verifies_each_transition_binding() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let old: AcquisitionManifest = storage::decode(include_bytes!("golden-manifest.json")).unwrap();
    let added = Ref {
        id: "proposal".into(),
        digest: old.baseline.digest.clone(),
    };
    let mut new = old.clone();
    new.proposals.push(added.clone());
    let marker = Recovery::new(
        Digest::of(b"old"),
        Digest::of(b"new"),
        old.active.clone(),
        None,
    );
    assert_eq!(marker.verify(&old, &new, &db, "reader").unwrap(), None);
    for field in [
        "resource",
        "baseline",
        "baseline_kind",
        "marker_active",
        "old_active",
        "activations",
        "proposal_append",
    ] {
        let mut before = old.clone();
        let mut after = new.clone();
        match field {
            "resource" => before.resource.id = "other".into(),
            "baseline" => before.baseline = added.clone(),
            "baseline_kind" => before.baseline_kind = BaselineKind::Local,
            "marker_active" => {}
            "old_active" => before.active = added.clone(),
            "activations" => after.activations.push(added.clone()),
            _ => after.proposals.push(added.clone()),
        }
        let marker = Recovery::new(
            Digest::of(b"old"),
            Digest::of(b"new"),
            if field == "marker_active" {
                added.clone()
            } else {
                old.active.clone()
            },
            None,
        );
        assert!(
            marker.verify(&before, &after, &db, "reader").is_err(),
            "{field}"
        );
    }
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n30_activation_recovery_verifies_each_transition_binding() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let old: AcquisitionManifest = storage::decode(include_bytes!("golden-manifest.json")).unwrap();
    let scope = old.resource.scope_tags.first().unwrap().parse().unwrap();
    db.grant("reader", &scope, Right::Read, "owner").unwrap();
    let other = Ref {
        id: "proposal".into(),
        digest: old.baseline.digest.clone(),
    };
    let gate: Handle = "00000000000000000000000001".parse().unwrap();
    for field in [
        "valid",
        "receipt",
        "status",
        "reason",
        "append",
        "previous_active",
        "payload_previous",
        "proposals",
    ] {
        let mut payload = Activation {
            proposal: other.clone(),
            gate,
            previous: old.active.clone(),
            restores: None,
        };
        if field == "payload_previous" {
            payload.previous = other.clone();
        }
        let bytes = storage::encode(&payload).unwrap();
        let handle = db.retain(&scope, &bytes, &[]).unwrap();
        let active = Ref {
            id: handle.to_string(),
            digest: Digest::of(&bytes),
        };
        let mut new = old.clone();
        new.active = active.clone();
        new.activations.push(active.clone());
        let mut before = old.clone();
        let mut event = Progress {
            receipt: handle,
            status: Status::Complete,
            reason: Reason::None,
        };
        match field {
            "receipt" => event.receipt = gate,
            "status" => event.status = Status::Partial,
            "reason" => event.reason = Reason::Revoked,
            "append" => before.activations.push(active),
            "previous_active" => before.active = other.clone(),
            "proposals" => {
                new.proposals.push(other.clone());
                new.proposals.push(other.clone());
            }
            _ => {}
        }
        let marker = Recovery::new(
            Digest::of(b"old"),
            Digest::of(b"new"),
            old.active.clone(),
            Some(event),
        );
        let result = marker.verify(&before, &new, &db, "reader");
        if field == "valid" {
            assert_eq!(result.unwrap(), Some(event));
        } else {
            assert!(result.is_err(), "{field}");
        }
    }
    drop(db);
    fs::remove_dir_all(root).unwrap();
}
