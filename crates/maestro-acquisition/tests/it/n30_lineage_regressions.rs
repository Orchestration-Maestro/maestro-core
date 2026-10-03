//! Digest-consistent payload substitutions must not break effective ancestry.

use super::{
    n30_support::{Events, Fixture, Grant},
    support,
};
use maestro_acquisition::{
    Ref, Refusal,
    adaptation::{AtomicCommit, ConfigurationWriter, LocalWriter, WriteError},
    policy::manifest::AcquisitionManifest,
    validate,
};
use maestro_kernel::{
    acquisition::{Handle, Receipts},
    artifact::Digest,
};
use serde_json::{Value, json};
use std::fs;

#[test]
fn n30_effective_lineage_verifies_each_binding() {
    for field in [
        "previous",
        "expected_active",
        "expected_baseline",
        "restores",
    ] {
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
        let active = writer
            .activate(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &proposal,
                fixture.gate,
            )
            .unwrap();
        let artifact = fixture
            .db
            .read(principal.id, active.id.parse().unwrap())
            .unwrap()
            .unwrap();
        let mut activation: Value = serde_json::from_slice(artifact.bytes()).unwrap();
        let scope = &fixture
            .manifest
            .resource
            .scope_tags
            .first()
            .unwrap()
            .parse()
            .unwrap();
        let mut links = vec![proposal.id.parse::<Handle>().unwrap(), fixture.gate];
        let wrong = Ref {
            id: "missing".into(),
            digest: Digest::of(b"wrong ancestry"),
        };
        if field.starts_with("expected") {
            let mut forged = fixture.proposal.clone();
            if field == "expected_active" {
                forged.expected_active = wrong;
            } else {
                forged.expected_baseline = wrong;
            }
            let bytes = serde_json::to_vec(&forged).unwrap();
            let handle = fixture.db.retain(scope, &bytes, &[forged.report]).unwrap();
            activation["proposal"] = json!(Ref {
                id: handle.to_string(),
                digest: Digest::of(&bytes)
            });
            *links.first_mut().unwrap() = handle;
        } else {
            activation[field] = json!(wrong);
        }
        let bytes = serde_json::to_vec(&activation).unwrap();
        let handle = fixture.db.retain(scope, &bytes, &links).unwrap();
        let reference = Ref {
            id: handle.to_string(),
            digest: Digest::of(&bytes),
        };
        let mut manifest: AcquisitionManifest = writer.current().unwrap();
        manifest.active = reference.clone();
        manifest.activations = vec![reference];
        let checked = validate(&fixture.catalog, &fixture.collection, &principal).unwrap();
        manifest.effective_digest =
            super::n30_support::effective_digest(checked.policy(), &manifest);
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert_eq!(
            writer.current(),
            Err(WriteError::Refused(Refusal::Invalid)),
            "{field}"
        );
    }
}

#[test]
fn n30_rollback_validates_target_before_pointer_exposure() {
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
    let second = writer
        .propose(&fixture.manifest.baseline, &a1, &proposal)
        .unwrap();
    let a2 = writer
        .activate(&fixture.manifest.baseline, &a1, &second, fixture.gate)
        .unwrap();
    let mut manifest = writer.current().unwrap();
    let scope = fixture
        .manifest
        .resource
        .scope_tags
        .first()
        .unwrap()
        .parse()
        .unwrap();
    let mut forged = fixture.proposal.clone();
    forged.expected_baseline.digest = Digest::of(b"wrong target baseline");
    let bytes = serde_json::to_vec(&forged).unwrap();
    let handle = fixture.db.retain(&scope, &bytes, &[forged.report]).unwrap();
    let bad_proposal = Ref {
        id: handle.to_string(),
        digest: Digest::of(&bytes),
    };
    let artifact = fixture
        .db
        .read(principal.id, a1.id.parse().unwrap())
        .unwrap()
        .unwrap();
    let mut activation: Value = serde_json::from_slice(artifact.bytes()).unwrap();
    activation["proposal"] = json!(bad_proposal);
    let bytes = serde_json::to_vec(&activation).unwrap();
    let handle = fixture
        .db
        .retain(&scope, &bytes, &[handle, fixture.gate])
        .unwrap();
    let target = Ref {
        id: handle.to_string(),
        digest: Digest::of(&bytes),
    };
    manifest.activations = vec![target.clone(), a2.clone()];
    let checked = validate(&fixture.catalog, &fixture.collection, &principal).unwrap();
    manifest.effective_digest = super::n30_support::effective_digest(checked.policy(), &manifest);
    let before = serde_json::to_vec(&manifest).unwrap();
    fs::write(root.join("manifest.json"), &before).unwrap();
    assert_eq!(
        writer.rollback(&a2, &target, fixture.gate),
        Err(WriteError::Refused(Refusal::Invalid))
    );
    assert_eq!(fs::read(root.join("manifest.json")).unwrap(), before);
    assert!(!root.join("recovery.json").exists());
}

#[test]
fn s6t_writer_and_snapshot_debug_redaction_has_names() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let context = fixture.context(&principal, &events, &Grant, &AtomicCommit);
    assert_eq!(format!("{context:?}"), "WriterContext { .. }");
    let writer =
        LocalWriter::open(&fixture.root.join("overlay"), context, &fixture.manifest).unwrap();
    assert_eq!(format!("{writer:?}"), "LocalWriter { .. }");
    let reader = super::n57_support::reader(
        &fixture.db,
        &fixture.collection,
        &fixture.catalog,
        &principal,
    );
    assert_eq!(format!("{reader:?}"), "SnapshotReader { .. }");
}

#[test]
fn s6t_direct_files_exact_byte_limit_and_overlimit() {
    use maestro_acquisition::{DirectFiles, LocalResource, ResourceSource};
    use maestro_knowledge::strict_json::MAX_BYTES;
    use std::collections::BTreeMap;
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let resource = fixture.catalog.0.get("policy").unwrap();
    let path = fixture.root.join("byte-boundary");
    let files = DirectFiles::new(BTreeMap::from([(
        resource.reference.id.clone(),
        LocalResource {
            path: path.clone(),
            admission: resource.admission.clone(),
        },
    )]));
    for count in [MAX_BYTES, MAX_BYTES + 2] {
        fs::write(&path, vec![b'x'; count]).unwrap();
        let read = files.read(&resource.reference, &principal);
        if count == MAX_BYTES {
            assert_eq!(read.unwrap().bytes.len(), MAX_BYTES);
        } else {
            assert_eq!(read.err(), Some(Refusal::Invalid));
        }
    }
}
