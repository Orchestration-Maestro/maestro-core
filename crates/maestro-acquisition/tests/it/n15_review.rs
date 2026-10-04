//! Permanent qualification and wire-shape regressions from the N15 review.
use super::n15_route_content_through_one_extensible_profile_registry::evidence;
use super::{n15_support as fixture, support};
use maestro_acquisition::extraction::registry::{
    LocalRegistry, Profile, ProfileSelection, RegistryUnavailable, definition_bytes,
};
use maestro_acquisition::{AdmissionStatus, ProfileRegistry};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};

/// Change only the containing shape, preserving all original field values.
fn positional(value: &Value, fields: &[&str]) -> Value {
    Value::Array(fields.iter().map(|field| value[*field].clone()).collect())
}

#[test]
fn n15_review_strict_shapes() {
    for case in [
        "processing",
        "decode_limits",
        "extractor",
        "qualification_state",
        "unknown_profile",
        "qualification_evidence",
        "output_schema",
        "qualification",
        "artifacts",
        "profiles",
    ] {
        let (mut collection, mut catalog, _) = fixture::registry_fixture();
        let mut registry = support::value(&catalog, "extraction");
        match case {
            "processing" => {
                registry["profiles"][1][case] = positional(
                    &registry["profiles"][1][case],
                    &["cleanup", "chunk", "dedup"],
                );
            }
            "decode_limits" => {
                registry["profiles"][1][case] = positional(
                    &registry["profiles"][1][case],
                    &[
                        "expanded_bytes",
                        "expansion_ratio",
                        "nested_levels",
                        "members",
                        "decoded_pixels",
                        "elapsed_ms",
                        "memory_bytes",
                        "xml_entities",
                    ],
                );
            }
            "extractor" | "output_schema" => {
                registry["profiles"][1][case] =
                    positional(&registry["profiles"][1][case], &["id", "digest"]);
            }
            "qualification_state" => registry["profiles"][1][case] = json!({"qualified":null}),
            "unknown_profile" | "qualification" => {
                registry[case] = positional(&registry[case], &["id", "digest"]);
            }
            "profiles" => registry[case][1] = json!([]),
            _ => {
                for reference in registry["profiles"][1][case].as_array_mut().unwrap() {
                    *reference = positional(reference, &["id", "digest"]);
                }
            }
        }
        // No resealing: malformed shapes must not decode to the approved definition.
        let reference = fixture::update(&mut collection, &mut catalog, &registry);
        assert_eq!(
            LocalRegistry::new(&catalog)
                .resolve(&reference, &support::principal(&support::scopes()))
                .err(),
            Some(RegistryUnavailable::Corrupt),
            "{case}",
        );
    }
}

#[test]
fn n15_dom_steps_are_strict_objects() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["profiles"][1]["structures"] = json!([
        {"kind":"observed","observation":{"kind":"dom","path":[{"tag":"p","attributes":[]}]}}
    ]);
    fixture::seal(&mut registry["profiles"][1], &mut catalog);
    registry["profiles"][1]["structures"][0]["observation"]["path"][0] = json!(["p", []]);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
}

#[test]
fn n15_review_transitive_qualification() {
    for parent in ["plugin", "output", "gold", "novel"] {
        for case in ["held", "revoked", "another-host"] {
            let (_, mut catalog, reference) = fixture::registry_fixture();
            let child = support::put(
                &mut catalog,
                "dependency",
                &json!({"synthetic":"dependency"}),
            );
            catalog
                .0
                .get_mut(parent)
                .unwrap()
                .admission
                .references
                .push(child);
            let admission = &mut catalog.0.get_mut("dependency").unwrap().admission;
            match case {
                "held" => admission.status = AdmissionStatus::Held,
                "revoked" => admission.status = AdmissionStatus::Revoked,
                _ => admission.platform = "another-host".into(),
            }
            let adapter = LocalRegistry::new(&catalog);
            match adapter.resolve(&reference, &support::principal(&support::scopes())) {
                Ok(checked) => assert!(
                    matches!(
                        adapter
                            .select(
                                &checked,
                                &evidence(b"NEW format"),
                                &[catalog.0["novel"].reference.clone()]
                            )
                            .unwrap(),
                        ProfileSelection::Held { .. }
                    ),
                    "{parent}: {case}"
                ),
                Err(error) => {
                    assert_eq!(error, RegistryUnavailable::Unqualified, "{parent}: {case}");
                }
            }
        }
    }
}

#[test]
fn n15_review_qualification_evidence_is_protected() {
    let (_, catalog, _) = fixture::registry_fixture();
    let profile: Profile =
        serde_json::from_value(support::value(&catalog, "extraction")["profiles"][1].clone())
            .unwrap();
    for case in ["id", "digest", "order", "length"] {
        let mut changed = profile.definition.clone();
        match case {
            "id" => changed.qualification_evidence[0].id = "alternate-gold".into(),
            "digest" => changed.qualification_evidence[0].digest = Digest::of(b"alternate-gold"),
            "order" => changed.qualification_evidence.reverse(),
            _ => {
                changed.qualification_evidence.pop();
            }
        }
        let original_bytes = definition_bytes(&profile.definition).unwrap();
        let changed_bytes = definition_bytes(&changed).unwrap();
        assert_ne!(original_bytes, changed_bytes, "{case}");
        assert_ne!(
            Digest::of(&original_bytes),
            Digest::of(&changed_bytes),
            "{case}"
        );
        assert!(
            !profile.definition.same_protected_fields(&changed),
            "{case}"
        );
    }
}

#[test]
fn n15_held_inventory_remains_visible_beside_selected_profile() {
    let (_, mut catalog, reference) = fixture::registry_fixture();
    fixture::unqualify(&mut catalog, "markdown");
    let adapter = LocalRegistry::new(&catalog);
    let resolved = adapter.resolve(&reference, &support::principal(&support::scopes()));
    assert!(resolved.is_ok(), "Held inventory must remain visible");
    let checked = resolved.unwrap();
    assert!(checked.profile(&catalog.0["markdown"].reference).is_some());
    assert!(matches!(
        adapter
            .select(
                &checked,
                &evidence(b"# text"),
                &[catalog.0["markdown"].reference.clone()]
            )
            .unwrap(),
        ProfileSelection::Held { .. }
    ));
    assert!(matches!(
        adapter
            .select(
                &checked,
                &evidence(b"NEW format"),
                &[catalog.0["novel"].reference.clone()]
            )
            .unwrap(),
        ProfileSelection::Selected { .. }
    ));
}

#[test]
fn n15_non_profile_registry_dependency_is_transitively_qualified() {
    let (_, mut catalog, reference) = fixture::registry_fixture();
    let shared = support::put(&mut catalog, "shared", &json!({"synthetic":"shared"}));
    let child = support::put(
        &mut catalog,
        "shared-child",
        &json!({"synthetic":"shared-child"}),
    );
    catalog
        .0
        .get_mut("shared")
        .unwrap()
        .admission
        .references
        .push(child);
    catalog
        .0
        .get_mut("extraction")
        .unwrap()
        .admission
        .references
        .push(shared);
    fixture::unqualify(&mut catalog, "shared-child");
    catalog.0.get_mut("shared-child").unwrap().admission.status = AdmissionStatus::Revoked;
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Unqualified)
    );
}
