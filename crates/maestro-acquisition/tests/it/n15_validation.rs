//! Independent boundary fixtures for registry guards and unmasked red proofs.
use super::{n15_support as fixture, support};
use maestro_acquisition::extraction::{
    detect::{DetectionEvidence, Observation},
    registry::{
        CheckedRegistry, DisabledRegistry, HeldReason, LocalRegistry, Profile, ProfileSelection,
        RegistryUnavailable, checked_resolve, checked_select, definition_bytes,
    },
};
use maestro_acquisition::{
    ImmutableResource, Principal, ProfileRegistry, Ref, Refusal, ResourceSource,
};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::cell::Cell;

/// The tested core resolve boundary, not a direct adapter call.
fn resolved(
    catalog: &support::Catalog,
    collection: &Value,
    reference: &Ref,
) -> Result<CheckedRegistry, RegistryUnavailable> {
    checked_resolve(
        &LocalRegistry::new(catalog),
        reference,
        &support::principal(&support::scopes()),
        &fixture::policy(collection, catalog),
    )
}

/// Minimal bounded evidence for content and structural neighbours.
fn sample(bytes: &[u8]) -> DetectionEvidence {
    let mut input =
        super::n15_route_content_through_one_extensible_profile_registry::evidence(bytes)
            .input()
            .clone();
    input.structures = vec![fixture::structure()];
    DetectionEvidence::new(input).unwrap()
}

#[test]
fn n15_ties_container_conflicts_and_structures_are_held() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["profiles"][2]["detectors"] = json!([{"kind":"magic","offset":0,"hex_bytes":"2320"}]);
    fixture::seal(&mut registry["profiles"][2], &mut catalog);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    let checked = resolved(&catalog, &collection, &reference).unwrap();
    let eligible = [
        catalog.0["markdown"].reference.clone(),
        catalog.0["novel"].reference.clone(),
    ];
    let adapter = LocalRegistry::new(&catalog);
    assert!(matches!(
        checked_select(&adapter, &checked, &sample(b"# text"), &eligible).unwrap(),
        ProfileSelection::Held {
            reason: HeldReason::Ambiguous,
            ..
        }
    ));
    registry["profiles"][2]["detectors"] = json!([{"kind":"container_member","name":"other-kind"}]);
    registry["profiles"][1]["structures"] = json!([
        {"kind":"all","children":[
            {"kind":"observed","observation":{"kind":"block","id":"heading"}},
            {"kind":"observed","observation":{"kind":"block","id":"table"}}
        ]}
    ]);
    fixture::seal(&mut registry["profiles"][1], &mut catalog);
    fixture::seal(&mut registry["profiles"][2], &mut catalog);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    let checked = resolved(&catalog, &collection, &reference).unwrap();
    let only_markdown = [catalog.0["markdown"].reference.clone()];
    assert!(matches!(
        checked_select(
            &LocalRegistry::new(&catalog),
            &checked,
            &sample(b"# text"),
            &only_markdown
        )
        .unwrap(),
        ProfileSelection::Held {
            reason: HeldReason::Unknown,
            ..
        }
    ));
    let mut input = sample(b"# text").input().clone();
    input
        .structures
        .push(Observation::Block { id: "table".into() });
    assert!(matches!(
        checked_select(
            &LocalRegistry::new(&catalog),
            &checked,
            &DetectionEvidence::new(input.clone()).unwrap(),
            &only_markdown
        )
        .unwrap(),
        ProfileSelection::Selected { .. }
    ));
    input.members.push("other-kind".into());
    let eligible = [
        catalog.0["markdown"].reference.clone(),
        catalog.0["novel"].reference.clone(),
    ];
    assert!(matches!(
        checked_select(
            &LocalRegistry::new(&catalog),
            &checked,
            &DetectionEvidence::new(input).unwrap(),
            &eligible
        )
        .unwrap(),
        ProfileSelection::Held {
            reason: HeldReason::Ambiguous,
            ..
        }
    ));
    let mut input = sample(b"# text").input().clone();
    input.structures.clear();
    assert!(matches!(
        checked_select(
            &LocalRegistry::new(&catalog),
            &checked,
            &DetectionEvidence::new(input).unwrap(),
            &eligible
        )
        .unwrap(),
        ProfileSelection::Held {
            reason: HeldReason::Unknown,
            ..
        }
    ));
}

#[test]
fn n15_definition_changes_cannot_reuse_approved_digest() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["profiles"][1]["admission_rules"] = json!(["forged-permission"]);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    assert_eq!(
        resolved(&catalog, &collection, &reference).err(),
        Some(RegistryUnavailable::Corrupt)
    );
}

#[test]
fn n15_strict_registry_shapes_and_scope_refuse() {
    for field in [
        "schema",
        "id",
        "empty",
        "duplicate",
        "scope",
        "owner",
        "qualification",
        "unknown",
        "extra",
        "profile-extra",
        "processing-extra",
        "missing-pin",
    ] {
        let (mut collection, mut catalog, _) = fixture::registry_fixture();
        let mut registry = support::value(&catalog, "extraction");
        match field {
            "schema" => registry["schema"] = json!("unknown/1"),
            "id" => registry["id"] = json!("another-registry"),
            "empty" => registry["profiles"] = json!([]),
            "duplicate" => registry["profiles"][2] = registry["profiles"][1].clone(),
            "scope" => registry["scope_tags"] = json!([]),
            "owner" => fixture::unqualify(&mut catalog, "owner"),
            "qualification" => fixture::unqualify(&mut catalog, "qualification"),
            "unknown" => registry["unknown_profile"] = json!(catalog.0["output"].reference),
            "missing-pin" => {
                registry["profiles"][1]["extractor"]
                    .as_object_mut()
                    .unwrap()
                    .remove("digest");
            }
            "profile-extra" => registry["profiles"][1]["unexpected"] = json!(true),
            "processing-extra" => registry["profiles"][1]["processing"]["unexpected"] = json!(true),
            _ => registry["unexpected"] = json!(true),
        }
        let reference = fixture::update(&mut collection, &mut catalog, &registry);
        // Resolve before policy validation, so its independent checks cannot mask registry guards.
        assert!(
            LocalRegistry::new(&catalog)
                .resolve(&reference, &support::principal(&support::scopes()))
                .is_err(),
            "{field}"
        );
    }
}

#[test]
fn n15_media_neighbours_hints_and_missing_capabilities_hold() {
    let (collection, catalog, reference) = fixture::registry_fixture();
    let checked = resolved(&catalog, &collection, &reference).unwrap();
    let adapter = LocalRegistry::new(&catalog);
    let eligible = [
        catalog.0["markdown"].reference.clone(),
        catalog.0["novel"].reference.clone(),
    ];
    for media in [
        "image/png",
        "audio/wav",
        "video/mp4",
        "application/pdf",
        "application/zip",
    ] {
        let mut input = sample(b"opaque").input().clone();
        input.declared_media = vec![media.into()];
        assert!(
            matches!(
                checked_select(
                    &adapter,
                    &checked,
                    &DetectionEvidence::new(input).unwrap(),
                    &eligible
                )
                .unwrap(),
                ProfileSelection::Held {
                    reason: HeldReason::Unknown,
                    ..
                }
            ),
            "{media}"
        );
    }
    let mut input = sample(b"NEW format").input().clone();
    input.capabilities.clear();
    assert!(matches!(
        checked_select(
            &adapter,
            &checked,
            &DetectionEvidence::new(input).unwrap(),
            &eligible
        )
        .unwrap(),
        ProfileSelection::Held {
            reason: HeldReason::Unknown,
            ..
        }
    ));
    assert!(matches!(
        checked_select(&adapter, &checked, &sample(b"# text"), &[]).unwrap(),
        ProfileSelection::Held { .. }
    ));
    assert_eq!(
        checked_select(
            &adapter,
            &checked,
            &sample(b"# text"),
            &[catalog.0["output"].reference.clone()]
        ),
        Err(RegistryUnavailable::Corrupt)
    );
    assert_eq!(
        checked_select(
            &adapter,
            &checked,
            &sample(b"# text"),
            &vec![catalog.0["markdown"].reference.clone(); 1001]
        ),
        Err(RegistryUnavailable::Corrupt)
    );
}

#[test]
fn n15_declared_media_alone_never_selects() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["profiles"][1]["detectors"] = json!([{"kind":"declared_media","media":"image/png"}]);
    fixture::seal(&mut registry["profiles"][1], &mut catalog);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    let checked = resolved(&catalog, &collection, &reference).unwrap();
    assert!(matches!(
        checked_select(
            &LocalRegistry::new(&catalog),
            &checked,
            &sample(b"opaque"),
            &[catalog.0["markdown"].reference.clone()]
        )
        .unwrap(),
        ProfileSelection::Held { .. }
    ));
}

#[test]
fn n15_installed_capability_alone_never_selects() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["profiles"][1]["detectors"] =
        json!([{"kind":"parser_capability","id":"synthetic-rust"}]);
    fixture::seal(&mut registry["profiles"][1], &mut catalog);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    let checked = resolved(&catalog, &collection, &reference).unwrap();
    assert!(matches!(
        checked_select(
            &LocalRegistry::new(&catalog),
            &checked,
            &sample(b"opaque"),
            &[catalog.0["markdown"].reference.clone()]
        )
        .unwrap(),
        ProfileSelection::Held { .. }
    ));
}

#[test]
fn n15_each_effective_protected_field_is_compared_and_digest_bound() {
    let (_, catalog, _) = fixture::registry_fixture();
    let original = support::value(&catalog, "extraction")["profiles"][1].clone();
    let profile: Profile = serde_json::from_value(original.clone()).unwrap();
    assert!(
        profile
            .definition
            .same_protected_fields(&profile.definition)
    );
    for field in [
        "decode_limits",
        "output_schema",
        "required_fidelity",
        "admission_rules",
        "artifacts",
        "platforms",
        "extractor",
        "qualification_evidence",
    ] {
        let mut changed = original.clone();
        match field {
            "decode_limits" => changed[field]["members"] = json!(2),
            "output_schema" => changed[field]["digest"] = json!(Digest::of(b"different-output")),
            "extractor" => changed[field] = json!(catalog.0["owner"].reference),
            "artifacts" | "qualification_evidence" => {
                changed[field] = json!([catalog.0["owner"].reference]);
            }
            _ => changed[field] = json!(["different"]),
        }
        let changed: Profile = serde_json::from_value(changed).unwrap();
        assert!(
            !profile
                .definition
                .same_protected_fields(&changed.definition),
            "{field}"
        );
        assert_ne!(
            Digest::of(&definition_bytes(&profile.definition).unwrap()),
            Digest::of(&definition_bytes(&changed.definition).unwrap()),
            "{field}"
        );
    }
}

/// A substitute tries to return a selected ref the immutable core never chose.
#[derive(Debug)]
struct Forged<'a> {
    local: LocalRegistry<'a>,
    reference: Ref,
}
impl ProfileRegistry for Forged<'_> {
    fn resolve(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<CheckedRegistry, RegistryUnavailable> {
        self.local.resolve(reference, principal)
    }
    fn select(
        &self,
        _checked: &CheckedRegistry,
        evidence: &DetectionEvidence,
        _eligible: &[Ref],
    ) -> Result<ProfileSelection, RegistryUnavailable> {
        Ok(ProfileSelection::Selected {
            profile: self.reference.clone(),
            evidence: evidence.input().evidence.clone(),
        })
    }
}
#[test]
fn n15_forged_adapter_selection_refuses() {
    let (collection, catalog, reference) = fixture::registry_fixture();
    let checked = resolved(&catalog, &collection, &reference).unwrap();
    let forged = Forged {
        local: LocalRegistry::new(&catalog),
        reference: catalog.0["novel"].reference.clone(),
    };
    assert_eq!(
        checked_select(
            &forged,
            &checked,
            &sample(b"# text"),
            &[catalog.0["markdown"].reference.clone()]
        ),
        Err(RegistryUnavailable::Corrupt)
    );
}

/// Counts only immutable-resource reads; no effect ports are exposed to selection.
#[derive(Debug)]
struct Counted<'a> {
    source: &'a support::Catalog,
    reads: Cell<usize>,
}
impl ResourceSource for Counted<'_> {
    fn read(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<ImmutableResource, Refusal> {
        self.reads.set(self.reads.get() + 1);
        self.source.read(reference, principal)
    }
}
#[test]
fn n15_selection_and_disable_have_zero_resource_or_effect_starts() {
    let (collection, catalog, reference) = fixture::registry_fixture();
    let counted = Counted {
        source: &catalog,
        reads: Cell::new(0),
    };
    let local = LocalRegistry::new(&counted);
    let policy = fixture::policy(&collection, &catalog);
    let checked = checked_resolve(
        &local,
        &reference,
        &support::principal(&support::scopes()),
        &policy,
    )
    .unwrap();
    counted.reads.set(0);
    checked_select(
        &local,
        &checked,
        &sample(b"# text"),
        &[catalog.0["markdown"].reference.clone()],
    )
    .unwrap();
    let disabled = DisabledRegistry;
    assert_eq!(
        checked_resolve(
            &disabled,
            &reference,
            &support::principal(&support::scopes()),
            &policy
        )
        .err(),
        Some(RegistryUnavailable::Disabled)
    );
    assert_eq!(
        checked_select(&disabled, &checked, &sample(b"# text"), &[]),
        Err(RegistryUnavailable::Disabled)
    );
    assert_eq!(counted.reads.get(), 0);
}
