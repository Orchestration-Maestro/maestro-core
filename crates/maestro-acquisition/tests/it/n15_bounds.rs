//! Independent shape/qualification guard neighbours, not parser-error mutants.
#![expect(
    clippy::indexing_slicing,
    reason = "synthetic profile fields must exist"
)]
use super::{n15_support as fixture, support};
use maestro_acquisition::extraction::{
    detect::{DetectionEvidence, Observation},
    registry::{
        LocalRegistry, ProfileSelection, RegistryUnavailable, checked_resolve, checked_select,
    },
};
use maestro_acquisition::{ProfileRegistry, Ref};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};

/// Re-pin changed profile bytes while preserving separate reviewed admission.
fn changed(field: &str, mut value: Value) -> (Value, support::Catalog, Ref) {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    support::bind(&mut value, &catalog);
    registry["profiles"][1][field] = value;
    fixture::seal(&mut registry["profiles"][1], &mut catalog);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    (collection, catalog, reference)
}

#[test]
fn n15_detector_shapes_have_independent_bounds() {
    let detectors = [
        json!({"kind":"magic","offset":65_536,"hex_bytes":"23"}),
        json!({"kind":"magic","offset":0,"hex_bytes":""}),
        json!({"kind":"magic","offset":0,"hex_bytes":"f"}),
        json!({"kind":"magic","offset":0,"hex_bytes":"ZZ"}),
        json!({"kind":"magic","offset":0,"hex_bytes":"00".repeat(257)}),
        json!({"kind":"container_member","name":""}),
        json!({"kind":"container_member","name":"x".repeat(4097)}),
        json!({"kind":"declared_media","media":""}),
        json!({"kind":"declared_media","media":"x".repeat(4097)}),
        json!({"kind":"parser_capability","id":"../unsafe"}),
    ];
    for detector in detectors {
        let (_, catalog, reference) = changed("detectors", json!([detector]));
        assert_eq!(
            LocalRegistry::new(&catalog)
                .resolve(&reference, &support::principal(&support::scopes()))
                .err(),
            Some(RegistryUnavailable::Corrupt)
        );
    }
    let (_, catalog, reference) = changed(
        "detectors",
        json!(vec![
            json!({"kind":"magic","offset":0,"hex_bytes":"23"});
            1001
        ]),
    );
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
}

#[test]
fn n15_structural_atoms_nodes_and_empty_boolean_refuse() {
    let atoms = [
        json!({"kind":"dom","path":[]}),
        json!({"kind":"dom","path":vec![json!({"tag":"p","attributes":[]});33]}),
        json!({"kind":"dom","path":[{
            "tag":"p","attributes":vec![json!({"name":"title","value":"x"});9]
        }]}),
        json!({"kind":"json","path":[]}),
        json!({"kind":"json","path":vec![json!({"kind":"index","value":0});33]}),
        json!({"kind":"json","path":[{"kind":"field","name":""}]}),
        json!({"kind":"block","id":"../bad"}),
        json!({"kind":"prefix","text":""}),
        json!({"kind":"prefix","text":"x".repeat(4097)}),
    ];
    for atom in atoms {
        let (_, catalog, reference) = changed(
            "structures",
            json!([{"kind":"observed","observation":atom}]),
        );
        assert_eq!(
            LocalRegistry::new(&catalog)
                .resolve(&reference, &support::principal(&support::scopes()))
                .err(),
            Some(RegistryUnavailable::Corrupt)
        );
    }
    for kind in ["all", "any"] {
        let (_, catalog, reference) = changed("structures", json!([{"kind":kind,"children":[]}]));
        assert_eq!(
            LocalRegistry::new(&catalog)
                .resolve(&reference, &support::principal(&support::scopes()))
                .err(),
            Some(RegistryUnavailable::Corrupt)
        );
    }
    let leaf = json!({"kind":"observed","observation":{"kind":"block","id":"heading"}});
    let (_, catalog, reference) = changed(
        "structures",
        json!([{"kind":"all","children":vec![leaf.clone();1000]}]),
    );
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
    let mut deep = leaf;
    for _ in 0..33 {
        deep = json!({"kind":"all","children":[deep]});
    }
    let (_, catalog, reference) = changed("structures", json!([deep]));
    assert_eq!(
        LocalRegistry::new(&catalog)
            .resolve(&reference, &support::principal(&support::scopes()))
            .err(),
        Some(RegistryUnavailable::Corrupt)
    );
}

#[test]
fn n15_profile_id_list_and_reference_bounds_refuse() {
    for (field, value) in [
        ("id", json!("../bad")),
        (
            "processing",
            json!({"chunk":"../bad","cleanup":[],"dedup":[]}),
        ),
        ("languages", json!(vec!["en"; 1001])),
        ("languages", json!(["../bad"])),
        (
            "qualification_evidence",
            json!(vec![json!({"id":"gold","digest":"0".repeat(64)}); 1001]),
        ),
    ] {
        let (_, catalog, reference) = changed(field, value);
        assert_eq!(
            LocalRegistry::new(&catalog)
                .resolve(&reference, &support::principal(&support::scopes()))
                .err(),
            Some(RegistryUnavailable::Corrupt)
        );
    }
}

#[test]
fn n15_profile_state_platform_fidelity_admission_and_artifact_are_required() {
    for (field, value) in [
        ("qualification_state", json!("proposed")),
        ("platforms", json!(["another-host"])),
        ("required_fidelity", json!([])),
        ("admission_rules", json!([])),
        ("artifacts", json!([])),
    ] {
        let (collection, catalog, reference) = changed(field, value);
        let registry = LocalRegistry::new(&catalog);
        let checked = checked_resolve(
            &registry,
            &reference,
            &support::principal(&support::scopes()),
            &fixture::policy(&collection, &catalog),
        )
        .unwrap();
        let evidence =
            super::n15_route_content_through_one_extensible_profile_registry::evidence(b"# text");
        assert!(
            matches!(
                checked_select(
                    &registry,
                    &checked,
                    &evidence,
                    &[catalog.0["markdown"].reference.clone()]
                )
                .unwrap(),
                ProfileSelection::Held { .. }
            ),
            "{field}"
        );
    }
}

#[test]
fn n15_literal_prefix_detects_content_not_metadata() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["profiles"][1]["detectors"] = json!([]);
    registry["profiles"][1]["structures"] = json!([
        {"kind":"observed", "observation":{"kind":"prefix", "text":"# "}}
    ]);
    fixture::seal(&mut registry["profiles"][1], &mut catalog);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    let adapter = LocalRegistry::new(&catalog);
    let checked = checked_resolve(
        &adapter,
        &reference,
        &support::principal(&support::scopes()),
        &fixture::policy(&collection, &catalog),
    )
    .unwrap();
    let make = super::n15_route_content_through_one_extensible_profile_registry::evidence;
    let eligible = [catalog.0["markdown"].reference.clone()];
    assert!(matches!(
        checked_select(&adapter, &checked, &make(b"# heading"), &eligible).unwrap(),
        ProfileSelection::Selected { .. }
    ));
    assert!(matches!(
        checked_select(&adapter, &checked, &make(b"opaque"), &eligible).unwrap(),
        ProfileSelection::Held { .. }
    ));
}

#[test]
fn n15_invalid_evidence_atoms_text_and_refs_refuse() {
    let original =
        super::n15_route_content_through_one_extensible_profile_registry::evidence(b"# text")
            .input()
            .clone();
    for field in ["text", "members", "structures", "assets"] {
        let mut input = original.clone();
        match field {
            "text" => input.text = "nul\0text".into(),
            "members" => input.members.push("x".repeat(4097)),
            "structures" => input.structures.push(Observation::Prefix {
                text: String::new(),
            }),
            _ => input.assets.push(Ref {
                id: "../bad".into(),
                digest: Digest::of(b"x"),
            }),
        }
        assert!(DetectionEvidence::new(input).is_err(), "{field}");
    }
}
