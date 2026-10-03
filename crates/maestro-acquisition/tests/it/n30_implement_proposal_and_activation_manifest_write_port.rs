//! Synthetic write-port conformance; no installed catalog admission is claimed.
use super::n30_support::{Crash, Events, Fixture, Grant};
use super::support;
use maestro_acquisition::policy::manifest::AcquisitionManifest;
use maestro_acquisition::{
    AdmissionStatus, DirectFiles, LocalResource, Ref, Refusal, ResourceSource,
    adaptation::{AtomicCommit, ConfigurationWriter, HeldAuthority, LocalWriter, WriteError},
    policy::manifest::BaselineKind,
};
use maestro_kernel::{acquisition::Receipts, artifact::Digest, filesystem, scope::Scope};
use std::{collections::BTreeMap, fs, sync::Barrier, thread};

#[test]
fn n30_direct_and_synthetic_share_roundtrip_without_trusted_writes() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let trusted = fixture.root.join("trusted");
    fs::create_dir(&trusted).unwrap();
    let mut resources = BTreeMap::new();
    for (id, resource) in &fixture.catalog.0 {
        let path = trusted.join(format!("{id}.json"));
        fs::write(&path, &resource.bytes).unwrap();
        resources.insert(
            id.clone(),
            LocalResource {
                path,
                admission: resource.admission.clone(),
            },
        );
    }
    let direct = DirectFiles::new(resources);
    for (source, kind) in [
        (&direct as &dyn ResourceSource, BaselineKind::Local),
        (
            &fixture.catalog as &dyn ResourceSource,
            BaselineKind::Catalog,
        ),
    ] {
        let mut initial = fixture.manifest.clone();
        initial.baseline_kind = kind;
        let root = fixture.root.join(format!("overlay-{kind:?}"));
        fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut context = fixture.context(&principal, &events, &Grant, &AtomicCommit);
        context.source = source;
        let writer = LocalWriter::open(&root, context, &initial).unwrap();
        let before = writer.current().unwrap();
        let port: &dyn ConfigurationWriter = &writer;
        let proposal = port
            .propose(&before.baseline, &before.active, &fixture.proposal)
            .unwrap();
        assert_eq!(writer.current().unwrap().active, before.active);
        let activation = port
            .activate(&before.baseline, &before.active, &proposal, fixture.gate)
            .unwrap();
        let after = writer.current().unwrap();
        assert_eq!(after.active, activation);
        assert_eq!(after.proposals, vec![proposal]);
        assert_eq!(after.activations, vec![activation]);
        assert_ne!(before.effective_digest, after.effective_digest);
        assert_eq!(after.baseline, before.baseline);
        assert_eq!(
            after.effective_digest,
            writer.current().unwrap().effective_digest
        );
    }
    for (id, resource) in &fixture.catalog.0 {
        assert_eq!(
            fs::read(trusted.join(format!("{id}.json"))).unwrap(),
            resource.bytes
        );
    }
    assert_eq!(events.0.lock().unwrap().len(), 2);
}

#[test]
fn n30_expected_baseline_and_active_cas_refuse_stale_writers() {
    for local in [false, true] {
        let fixture = Fixture::new();
        let direct = fixture.direct();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let writer = LocalWriter::open(
            &fixture.root.join("overlay"),
            fixture.context_source(
                &principal,
                &events,
                (&Grant, &AtomicCommit),
                if local { &direct } else { &fixture.catalog },
            ),
            &fixture.manifest,
        )
        .unwrap();
        let port: &dyn ConfigurationWriter = &writer;
        let proposal = port
            .propose(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &fixture.proposal,
            )
            .unwrap();
        let wrong = Ref {
            id: "other".into(),
            digest: Digest::of(b"other"),
        };
        assert_eq!(
            port.activate(&wrong, &fixture.manifest.active, &proposal, fixture.gate),
            Err(WriteError::Conflict)
        );
        port.activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            fixture.gate,
        )
        .unwrap();
        assert_eq!(
            port.activate(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &proposal,
                fixture.gate
            ),
            Err(WriteError::Conflict)
        );
        assert_eq!(
            port.propose(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &fixture.proposal
            ),
            Err(WriteError::Conflict)
        );
        assert_eq!(events.0.lock().unwrap().len(), 1);
    }
}

#[test]
fn n30_concurrent_activations_have_exactly_one_winner() {
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
    let barrier = Barrier::new(2);
    thread::scope(|scope| {
        let run = || {
            let writer = LocalWriter::open(
                &root,
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &fixture.manifest,
            )
            .unwrap();
            barrier.wait();
            writer.activate(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &proposal,
                fixture.gate,
            )
        };
        let first = scope.spawn(run);
        let second = scope.spawn(run);
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(results.iter().filter(|item| item.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|item| **item == Err(WriteError::Conflict))
                .count(),
            1
        );
    });
    assert_eq!(writer.current().unwrap().activations.len(), 1);
}

#[test]
fn n30_crashes_before_and_after_commit_recover_only_complete_states() {
    for local in [false, true] {
        for after_commit in [false, true] {
            let fixture = Fixture::new();
            let direct = fixture.direct();
            let principal = support::principal(&fixture.scopes);
            let events = Events::default();
            let root = fixture.root.join("overlay");
            let writer = LocalWriter::open(
                &root,
                fixture.context_source(
                    &principal,
                    &events,
                    (&Grant, &AtomicCommit),
                    if local { &direct } else { &fixture.catalog },
                ),
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
            let crash = Crash(after_commit);
            let writer = LocalWriter::open(
                &root,
                fixture.context_source(
                    &principal,
                    &events,
                    (&Grant, &crash),
                    if local { &direct } else { &fixture.catalog },
                ),
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
            assert!(events.0.lock().unwrap().is_empty());
            drop(writer);
            let recovered = LocalWriter::open(
                &root,
                fixture.context_source(
                    &principal,
                    &events,
                    (&Grant, &AtomicCommit),
                    if local { &direct } else { &fixture.catalog },
                ),
                &fixture.manifest,
            )
            .unwrap();
            let state = recovered.current().unwrap();
            assert_eq!(state.activations.len(), usize::from(after_commit));
            assert_eq!(events.0.lock().unwrap().len(), usize::from(after_commit));
            assert_eq!(state.active == fixture.manifest.active, !after_commit);
            assert!(!root.join("recovery.json").exists());
            drop(recovered);
            LocalWriter::open(
                &root,
                fixture.context_source(
                    &principal,
                    &events,
                    (&Grant, &AtomicCommit),
                    if local { &direct } else { &fixture.catalog },
                ),
                &fixture.manifest,
            )
            .unwrap();
            assert_eq!(events.0.lock().unwrap().len(), usize::from(after_commit));
        }
    }
}

#[test]
fn n30_substituted_revoked_missing_baseline_never_falls_back() {
    let faults = [
        "substituted",
        "revoked",
        "missing",
        "platform",
        "admission",
        "reference",
    ];
    for (local, fault) in [false, true]
        .into_iter()
        .flat_map(|local| faults.into_iter().map(move |fault| (local, fault)))
    {
        // DirectFiles binds the returned identity to the request by construction.
        // A substituted returned Ref is an adversarial ResourceSource-only case.
        if local && fault == "reference" {
            continue;
        }
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
            writer
                .propose(
                    &fixture.manifest.baseline,
                    &fixture.manifest.active,
                    &fixture.proposal,
                )
                .unwrap();
        }
        if fault == "missing" {
            fixture.catalog.0.remove("policy");
        } else {
            let resource = fixture.catalog.0.get_mut("policy").unwrap();
            match fault {
                "substituted" => resource.bytes.push(b' '),
                "revoked" => resource.admission.status = AdmissionStatus::Revoked,
                "platform" => resource.admission.platform = "unqualified-platform".into(),
                "admission" => resource.admission.digest = Digest::of(b"different"),
                "reference" => resource.reference.id = "substitute".into(),
                _ => {}
            }
        }
        let direct = fixture.direct();
        let principal = support::principal(&fixture.scopes);
        assert!(
            LocalWriter::open(
                &root,
                fixture.context_source(
                    &principal,
                    &events,
                    (&Grant, &AtomicCommit),
                    if local { &direct } else { &fixture.catalog }
                ),
                &fixture.manifest
            )
            .is_err(),
            "{fault}"
        );
        assert!(events.0.lock().unwrap().is_empty());
    }
}

#[test]
fn n30_held_gate_and_inaccessible_evidence_do_not_expose_activation() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let root = fixture.root.join("overlay");
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &HeldAuthority, &AtomicCommit),
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
    assert_eq!(
        writer.activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            fixture.gate
        ),
        Err(WriteError::Held)
    );
    let raw: AcquisitionManifest =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(raw.active, fixture.manifest.active);
    assert_eq!(writer.current().unwrap().active, fixture.manifest.active);
    drop(writer);
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let denied_scope: Scope = "workspace/default/collection/denied-gate".parse().unwrap();
    let denied_gate = fixture
        .db
        .retain(&denied_scope, b"existing but inaccessible gate", &[])
        .unwrap();
    assert_eq!(
        writer.activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            denied_gate
        ),
        Err(WriteError::Refused(Refusal::Access))
    );
    let raw: AcquisitionManifest =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(raw.active, fixture.manifest.active);
    assert!(events.0.lock().unwrap().is_empty());
}

#[test]
fn n30_overlay_rebind_and_trusted_bundle_as_pointer_are_refused() {
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
    let mut rebound = fixture.manifest.clone();
    rebound.baseline.digest = Digest::of(b"replacement");
    assert!(
        LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &rebound
        )
        .is_err()
    );
    let bytes = fixture.catalog.0["policy"].bytes.clone();
    fs::write(root.join("manifest.json"), &bytes).unwrap();
    assert!(
        LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &fixture.manifest
        )
        .is_err()
    );
    assert_eq!(fs::read(root.join("manifest.json")).unwrap(), bytes);
}

#[test]
fn n30_overlay_root_is_protected() {
    let fixture = Fixture::new();
    let root = fixture.root.join("overlay");
    assert!(filesystem::protected_root(&root).is_ok());
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let link = fixture.root.join("link");
        symlink(&root, &link).unwrap();
        assert!(
            LocalWriter::open(
                &link,
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &fixture.manifest
            )
            .is_err()
        );
        fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(
            LocalWriter::open(
                &root,
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &fixture.manifest
            )
            .is_err()
        );
    }
}
