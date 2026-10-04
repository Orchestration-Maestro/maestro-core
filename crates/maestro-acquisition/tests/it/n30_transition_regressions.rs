//! Recovery must prove an exact pointer transition before delivery.
use super::{
    n30_support::{Crash, Events, Fixture, Grant},
    support,
};
use maestro_acquisition::adaptation::{AtomicCommit, ConfigurationWriter, LocalWriter, WriteError};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::strict_json::MAX_BYTES;
use serde_json::{Value, json};
use std::{fs, path::Path};

#[test]
fn n30_recovery_requires_exact_old_pointer_record() {
    for corruption in ["missing", "digest", "strict", "bounded"] {
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
        let path = root.join("recovery-old.json");
        match corruption {
            "digest" => {
                let mut bytes = fs::read(&path).unwrap();
                bytes.push(b' ');
                fs::write(&path, bytes).unwrap();
            }
            "strict" => {
                let original = String::from_utf8(fs::read(&path).unwrap()).unwrap();
                let before: Value = serde_json::from_str(&original).unwrap();
                let repeated = format!(
                    "\"baseline\":{{\"id\":{},",
                    serde_json::to_string(&before["baseline"]["id"]).unwrap()
                );
                let bytes = original
                    .replacen("\"baseline\":{", &repeated, 1)
                    .into_bytes();
                forge_old_record(&root, &bytes);
            }
            "bounded" => {
                let mut bytes = fs::read(&path).unwrap();
                bytes.resize(MAX_BYTES + 1, b' ');
                forge_old_record(&root, &bytes);
            }
            _ => {
                fs::remove_file(&path).unwrap();
            }
        }
        assert!(
            LocalWriter::open(
                &root,
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &fixture.manifest
            )
            .is_err(),
            "corruption={corruption}"
        );
        assert!(root.join("recovery.json").exists());
        assert!(events.0.lock().unwrap().is_empty());
    }
}

fn forge_old_record(root: &Path, bytes: &[u8]) {
    fs::write(root.join("recovery-old.json"), bytes).unwrap();
    let marker_path = root.join("recovery.json");
    let mut marker: Value = serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    *marker.get_mut("old").unwrap() = json!(Digest::of(bytes));
    fs::write(marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();
}

#[test]
fn n30_recovery_refuses_relabelled_activation_as_proposal() {
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
    assert!(
        writer
            .activate(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &reference,
                fixture.gate
            )
            .is_err()
    );
    drop(writer);
    let mut marker: Value =
        serde_json::from_slice(&fs::read(root.join("recovery.json")).unwrap()).unwrap();
    marker["kind"] = json!("proposal");
    marker.as_object_mut().unwrap().remove("event");
    marker.as_object_mut().unwrap().remove("previous_active");
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    marker["active"] = manifest["active"].clone();
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
    assert!(events.0.lock().unwrap().is_empty());
}

#[test]
fn n30_recovery_activation_must_append_exactly_once() {
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
    let old = fs::read(root.join("manifest.json")).unwrap();
    let crash = Crash(true);
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &crash),
        &fixture.manifest,
    )
    .unwrap();
    assert!(
        writer
            .activate(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &reference,
                fixture.gate
            )
            .is_err()
    );
    drop(writer);
    // Forge a digest-consistent old pointer already containing the new activation.
    let mut before: Value = serde_json::from_slice(&old).unwrap();
    let after: Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    before["activations"] = after["activations"].clone();
    let forged = serde_json::to_vec(&before).unwrap();
    fs::write(root.join("recovery-old.json"), &forged).unwrap();
    let mut marker: Value =
        serde_json::from_slice(&fs::read(root.join("recovery.json")).unwrap()).unwrap();
    marker["old"] = json!(Digest::of(&forged));
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
    assert!(events.0.lock().unwrap().is_empty());
}

#[test]
fn n30_crash_after_old_record_before_marker_keeps_pointer() {
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
    drop(writer);
    let before = fs::read(root.join("manifest.json")).unwrap();
    fs::write(root.join("recovery-old.json"), &before).unwrap();
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    assert_eq!(fs::read(root.join("manifest.json")).unwrap(), before);
    writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &fixture.proposal,
        )
        .unwrap();
    assert!(!root.join("recovery-old.json").exists());
    assert!(events.0.lock().unwrap().is_empty());
}
