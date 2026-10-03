//! Selection predicates with qualified, pinned positive and negative neighbours.
use super::{n15_support as fixture, support};
use maestro_acquisition::ProfileRegistry;
use maestro_acquisition::extraction::{
    detect::DetectionEvidence,
    registry::{LocalRegistry, ProfileSelection, checked_resolve, checked_select},
};
use serde_json::json;

#[test]
fn n15_safe_profile_never_becomes_a_selected_default() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["profiles"][0]["detectors"] = json!([{"kind":"magic","offset":0,"hex_bytes":"2320"}]);
    fixture::seal(&mut registry["profiles"][0], &mut catalog);
    support::bind(&mut registry, &catalog);
    let reference = fixture::update(&mut collection, &mut catalog, &registry);
    let adapter = LocalRegistry::new(&catalog);
    let checked = adapter
        .resolve(&reference, &support::principal(&support::scopes()))
        .unwrap();
    let sample =
        super::n15_route_content_through_one_extensible_profile_registry::evidence(b"# text");
    assert!(matches!(
        adapter
            .select(&checked, &sample, &[catalog.0["safe"].reference.clone()])
            .unwrap(),
        ProfileSelection::Held { .. }
    ));
}

#[test]
fn n15_media_hint_corroborates_but_cannot_override_content() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["profiles"][1]["detectors"] = json!([
        {"kind":"magic","offset":0,"hex_bytes":"2320"},
        {"kind":"declared_media","media":"text/markdown"}
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
    let mut input = make(b"# text").input().clone();
    input.declared_media = vec!["text/markdown".into()];
    assert!(matches!(
        checked_select(
            &adapter,
            &checked,
            &DetectionEvidence::new(input).unwrap(),
            &eligible
        )
        .unwrap(),
        ProfileSelection::Selected { .. }
    ));
    assert!(matches!(
        checked_select(&adapter, &checked, &make(b"# text"), &eligible).unwrap(),
        ProfileSelection::Held { .. }
    ));
    let mut input = make(b"opaque").input().clone();
    input.declared_media = vec!["text/markdown".into()];
    assert!(matches!(
        checked_select(
            &adapter,
            &checked,
            &DetectionEvidence::new(input).unwrap(),
            &eligible
        )
        .unwrap(),
        ProfileSelection::Held { .. }
    ));
}

#[test]
fn n15_structural_disjunction_obeys_content_observations() {
    let (mut collection, mut catalog, _) = fixture::registry_fixture();
    let mut registry = support::value(&catalog, "extraction");
    registry["profiles"][1]["structures"] = json!([{"kind":"any","children":[
        {"kind":"observed","observation":{"kind":"block","id":"heading"}},
        {"kind":"observed","observation":{"kind":"block","id":"table"}}
    ]}]);
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
    let mut input = make(b"# text").input().clone();
    input.structures = vec![fixture::structure()];
    assert!(matches!(
        checked_select(
            &adapter,
            &checked,
            &DetectionEvidence::new(input).unwrap(),
            &eligible
        )
        .unwrap(),
        ProfileSelection::Selected { .. }
    ));
    assert!(matches!(
        checked_select(&adapter, &checked, &make(b"# text"), &eligible).unwrap(),
        ProfileSelection::Held { .. }
    ));
}
