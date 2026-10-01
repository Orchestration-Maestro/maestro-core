//! One resolve/select contract over local, synthetic catalog and disabled ports.
use super::{n15_support as fixture, support};
use maestro_acquisition::extraction::{
    detect::{DetectionEvidence, EvidenceInput},
    registry::{
        DisabledRegistry, HeldReason, LocalRegistry, Profile, ProfileSelection,
        RegistryUnavailable, checked_resolve, checked_select, definition_bytes,
    },
};
use maestro_acquisition::{ProfileRegistry, Ref, Refusal};
use maestro_kernel::artifact::Digest;
use serde_json::json;
use std::{fs, ptr};

/// A bounded capture sample with misleading filename metadata.
pub(super) fn evidence(bytes: &[u8]) -> DetectionEvidence {
    DetectionEvidence::new(EvidenceInput {
        sample: bytes.to_vec(),
        declared_media: vec!["image/png".into()],
        members: vec![],
        structures: vec![],
        capabilities: vec!["synthetic-rust".into()],
        encrypted: false,
        malformed: false,
        text: "safe partial".into(),
        metadata: vec!["filename=misleading.png".into()],
        assets: vec![Ref {
            id: "retained-capture".into(),
            digest: Digest::of(bytes),
        }],
        evidence: vec![],
    })
    .unwrap()
}

#[test]
fn n15_shared_resolve_select_contract() {
    let (collection, catalog, reference) = fixture::registry_fixture();
    let policy = fixture::policy(&collection, &catalog);
    let scopes = support::scopes();
    let principal = support::principal(&scopes);
    let (directory, files) = fixture::direct(&catalog);
    let local = LocalRegistry::new(&files);
    let substitute = LocalRegistry::new(&catalog);
    let disabled = DisabledRegistry;
    let eligible = vec![
        catalog.0["markdown"].reference.clone(),
        catalog.0["novel"].reference.clone(),
    ];
    for registry in [&local as &dyn ProfileRegistry, &substitute, &disabled] {
        let resolved = checked_resolve(registry, &reference, &principal, &policy);
        if ptr::eq(registry, &disabled as &dyn ProfileRegistry) {
            assert_eq!(resolved.err(), Some(RegistryUnavailable::Disabled));
            let checked = checked_resolve(&local, &reference, &principal, &policy).unwrap();
            assert_eq!(
                checked_select(registry, &checked, &evidence(b"# heading"), &eligible),
                Err(RegistryUnavailable::Disabled)
            );
            continue;
        }
        let checked = resolved.unwrap();
        let raw = registry.resolve(&reference, &principal).unwrap();
        assert_eq!(
            registry
                .select(&raw, &evidence(b"# heading"), &eligible)
                .unwrap(),
            checked_select(registry, &checked, &evidence(b"# heading"), &eligible).unwrap()
        );
        for (bytes, id) in [
            (b"# heading".as_slice(), "markdown"),
            (b"NEW format".as_slice(), "novel"),
        ] {
            let selected = checked_select(registry, &checked, &evidence(bytes), &eligible).unwrap();
            assert_eq!(
                selected,
                ProfileSelection::Selected {
                    profile: catalog.0[id].reference.clone(),
                    evidence: vec![]
                }
            );
        }
        let unknown = checked_select(registry, &checked, &evidence(b"opaque"), &eligible).unwrap();
        assert_eq!(
            unknown,
            ProfileSelection::Held {
                reason: HeldReason::Unknown,
                safe_profile: catalog.0["safe"].reference.clone(),
                evidence: vec![],
                partial: evidence(b"opaque").partial().clone()
            }
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn n15_encrypted_malformed_conflicting_and_unqualified_hold() {
    let (collection, mut catalog, reference) = fixture::registry_fixture();
    let policy = fixture::policy(&collection, &catalog);
    fixture::unqualify(&mut catalog, "markdown");
    let registry = LocalRegistry::new(&catalog);
    let checked = checked_resolve(
        &registry,
        &reference,
        &support::principal(&support::scopes()),
        &policy,
    )
    .unwrap();
    let eligible = vec![catalog.0["markdown"].reference.clone()];
    assert!(matches!(
        checked_select(&registry, &checked, &evidence(b"# text"), &eligible).unwrap(),
        ProfileSelection::Held {
            reason: HeldReason::Unknown,
            ..
        }
    ));
    for flag in ["encrypted", "malformed"] {
        let mut input = evidence(b"NEW content").input().clone();
        input.encrypted = flag == "encrypted";
        input.malformed = flag == "malformed";
        let selection = checked_select(
            &registry,
            &checked,
            &DetectionEvidence::new(input).unwrap(),
            &[catalog.0["novel"].reference.clone()],
        )
        .unwrap();
        assert!(matches!(
            selection,
            ProfileSelection::Held {
                reason: HeldReason::Unknown,
                ..
            }
        ));
    }
}

#[test]
fn n15_forged_resource_bytes_admission_and_identity_refuse() {
    let (collection, catalog, reference) = fixture::registry_fixture();
    let policy = fixture::policy(&collection, &catalog);
    for field in ["bytes", "admission", "identity", "platform", "status"] {
        let mut forged = catalog.clone();
        let resource = forged.0.get_mut("extraction").unwrap();
        match field {
            "bytes" => resource.bytes.push(b' '),
            "admission" => resource.admission.digest = Digest::of(b"forged"),
            "identity" => resource.reference.id = "forged".into(),
            "platform" => resource.admission.platform = "wrong-platform".into(),
            _ => resource.admission.status = maestro_acquisition::AdmissionStatus::Proposed,
        }
        assert!(
            checked_resolve(
                &LocalRegistry::new(&forged),
                &reference,
                &support::principal(&support::scopes()),
                &policy
            )
            .is_err(),
            "{field}"
        );
    }
}

#[test]
fn n15_missing_gold_capability_and_artifact_never_qualify() {
    let (collection, catalog, reference) = fixture::registry_fixture();
    let policy = fixture::policy(&collection, &catalog);
    for id in ["gold", "threshold", "plugin"] {
        let mut missing = catalog.clone();
        missing
            .0
            .get_mut(id)
            .unwrap()
            .admission
            .capabilities
            .clear();
        let registry = LocalRegistry::new(&missing);
        let result = checked_resolve(
            &registry,
            &reference,
            &support::principal(&support::scopes()),
            &policy,
        );
        if let Ok(checked) = result {
            assert!(
                matches!(
                    checked_select(
                        &registry,
                        &checked,
                        &evidence(b"NEW format"),
                        &[catalog.0["novel"].reference.clone()]
                    )
                    .unwrap(),
                    ProfileSelection::Held { .. }
                ),
                "{id}"
            );
        }
    }
    let mut missing = catalog.clone();
    missing.0.remove("plugin");
    assert_eq!(
        checked_resolve(
            &LocalRegistry::new(&missing),
            &reference,
            &support::principal(&support::scopes()),
            &policy
        )
        .err(),
        Some(RegistryUnavailable::Missing)
    );
}

#[test]
fn n15_profile_digest_golden_and_canonical_order() {
    let (_, catalog, _) = fixture::registry_fixture();
    let mut value = support::value(&catalog, "extraction")["profiles"][0].clone();
    value["platforms"] = json!(["golden-platform"]);
    let profile: Profile = serde_json::from_value(value.clone()).unwrap();
    let digest = Digest::of(&definition_bytes(&profile.definition).unwrap());
    assert_eq!(
        digest.as_str(),
        "679230578236d8ad1e7b5a8a6d268452015a76be42c43f0739fbe48d728c7d41"
    );
    let fields: Vec<_> = value
        .as_object()
        .unwrap()
        .iter()
        .rev()
        .map(|(key, value)| {
            format!(
                "{} : {}",
                serde_json::to_string(key).unwrap(),
                serde_json::to_string(value).unwrap()
            )
        })
        .collect();
    // Build raw JSON, so key order changes even without serde's preserve_order.
    let reordered = format!("{{\n{}\n}}", fields.join(",\n"));
    assert_ne!(reordered, serde_json::to_string_pretty(&value).unwrap());
    let decoded: Profile = serde_json::from_str(&reordered).unwrap();
    assert_eq!(
        definition_bytes(&profile.definition).unwrap(),
        definition_bytes(&decoded.definition).unwrap()
    );
    let mut changed = value;
    changed["admission_rules"] = json!(["different"]);
    let changed: Profile = serde_json::from_value(changed).unwrap();
    assert_ne!(
        digest,
        Digest::of(&definition_bytes(&changed.definition).unwrap())
    );
}

#[test]
fn n15_evidence_bounds_refuse_without_effects() {
    let original = evidence(b"# text").input().clone();
    for field in [
        "sample",
        "text",
        "metadata",
        "members",
        "structures",
        "capabilities",
        "assets",
        "evidence",
    ] {
        let mut input = original.clone();
        match field {
            "sample" => input.sample = vec![0; 65_537],
            "text" => input.text = "x".repeat(4097),
            "metadata" => input.metadata = vec!["x".into(); 1001],
            "members" => input.members = vec!["x".into(); 1001],
            "structures" => input.structures = vec![fixture::structure(); 1001],
            "capabilities" => input.capabilities = vec!["x".into(); 1001],
            "assets" => {
                input.assets = vec![
                    Ref {
                        id: "x".into(),
                        digest: Digest::of(b"x")
                    };
                    1001
                ];
            }
            _ => {
                input.evidence = vec![
                    Ref {
                        id: "x".into(),
                        digest: Digest::of(b"x")
                    };
                    1001
                ];
            }
        }
        assert_eq!(
            DetectionEvidence::new(input).err(),
            Some(Refusal::Invalid),
            "{field}"
        );
    }
}
