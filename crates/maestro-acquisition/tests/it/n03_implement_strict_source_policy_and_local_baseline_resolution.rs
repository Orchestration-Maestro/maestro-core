//! N03: malformed policy closure must refuse before caller-side effects.
use super::support::{self, Catalog};
use maestro_acquisition::{AdmissionStatus, DirectFiles, LocalResource, PolicySource, Refusal};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::collection::Declaration;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

/// Execute the real policy boundary, starting synthetic effects only on admission.
fn resolve(
    collection: &Value,
    catalog: &Catalog,
) -> Result<maestro_acquisition::CheckedPolicy, Refusal> {
    let scopes = support::scopes();
    let declaration: Declaration = collection
        .to_string()
        .parse()
        .map_err(|_| Refusal::Invalid)?;
    catalog.resolve(&declaration, &support::principal(&scopes))
}

/// The boundary under test must not give the caller a launchable policy.
fn refuses(collection: &Value, catalog: &Catalog) {
    let mut transport_starts = 0;
    let mut session_starts = 0;
    let result = resolve(collection, catalog);
    if result.is_ok() {
        transport_starts += 1;
        session_starts += 1;
    }
    assert!(result.is_err(), "unexpected checked policy: {result:?}");
    assert_eq!((transport_starts, session_starts), (0, 0));
}

#[test]
fn n03_reviewed_local_and_synthetic_catalog_bind_exact_digests() {
    let (collection, catalog) = support::fixture();
    let checked = resolve(&collection, &catalog).unwrap();
    assert_eq!(checked.policy().resource.id, "policy");
    assert_eq!(checked.reference(), &catalog.0["policy"].reference);
    let scratch = maestro_test_scratch::scratch_directory().unwrap();
    let mut resources = BTreeMap::new();
    for (id, resource) in &catalog.0 {
        let path = scratch.join(format!("{id}.json"));
        fs::write(&path, &resource.bytes).unwrap();
        resources.insert(
            id.clone(),
            LocalResource {
                path,
                admission: resource.admission.clone(),
            },
        );
    }
    let files = DirectFiles::new(resources);
    let declaration: Declaration = collection.to_string().parse().unwrap();
    let scopes = support::scopes();
    let local = files
        .resolve(&declaration, &support::principal(&scopes))
        .unwrap();
    assert_eq!(local.reference(), checked.reference());
    assert_eq!(local.policy(), checked.policy());
    fs::write(scratch.join("http.json"), b"{}").unwrap();
    assert!(
        files
            .resolve(&declaration, &support::principal(&scopes))
            .is_err()
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn n03_collection_versions_do_not_silently_authorize_acquisition() {
    let (mut collection, catalog) = support::fixture();
    collection["source_policy"] = Value::Null;
    let v2: Declaration = collection.to_string().parse().unwrap();
    refuses(&collection, &catalog);
    let text = serde_json::to_string(&v2).unwrap();
    assert_eq!(
        text,
        serde_json::to_string(&text.parse::<Declaration>().unwrap()).unwrap()
    );
    collection.as_object_mut().unwrap().remove("source_policy");
    assert!(collection.to_string().parse::<Declaration>().is_err());
    collection["schema"] = json!("maestro-collection/1");
    let v1: Declaration = collection.to_string().parse().unwrap();
    assert_eq!(
        serde_json::to_string(&v1).unwrap(),
        serde_json::to_string(
            &serde_json::to_string(&v1)
                .unwrap()
                .parse::<Declaration>()
                .unwrap()
        )
        .unwrap()
    );
    refuses(&collection, &catalog);
    collection["source_policy"] = json!(catalog.0["policy"].reference);
    assert!(collection.to_string().parse::<Declaration>().is_err());
    collection["schema"] = json!("maestro-collection/2");
    assert!(resolve(&collection, &catalog).is_ok());
}

#[test]
fn n03_duplicate_unknown_and_parser_ceiling_refusals() {
    let (mut collection, mut catalog) = support::fixture();
    let original = catalog.0["policy"].bytes.clone();
    let text = String::from_utf8(original.clone()).unwrap();
    for bytes in [
        text.replace(
            "\"agent\":\"maestro\"",
            "\"agent\":\"maestro\",\"agent\":\"other\"",
        )
        .into_bytes(),
        text.replace(
            "\"agent\":\"maestro\"",
            "\"agent\":\"maestro\",\"script\":\"run\"",
        )
        .into_bytes(),
        vec![b' '; 4 * 1024 * 1024 + 1],
        format!("{}0{}", "[".repeat(33), "]".repeat(33)).into_bytes(),
        format!("[{}]", vec!["0"; 10_001].join(",")).into_bytes(),
        format!(
            "{{\"a\":[{}],\"b\":[{}],\"c\":[{}]}}",
            vec!["0"; 7_000].join(","),
            vec!["0"; 7_000].join(","),
            vec!["0"; 7_000].join(",")
        )
        .into_bytes(),
    ] {
        let resource = catalog.0.get_mut("policy").unwrap();
        resource.reference.digest = Digest::of(&bytes);
        resource.admission.digest = resource.reference.digest.clone();
        resource.bytes = bytes;
        support::bind(&mut collection, &catalog);
        refuses(&collection, &catalog);
    }
}

#[test]
fn n03_missing_substituted_unreviewed_and_private_resources_refuse() {
    for id in [
        "policy",
        "decisions",
        "http",
        "adapter",
        "qualification",
        "extraction",
        "markdown",
    ] {
        for status in [
            AdmissionStatus::Proposed,
            AdmissionStatus::Held,
            AdmissionStatus::Revoked,
        ] {
            let (collection, mut catalog) = support::fixture();
            catalog.0.get_mut(id).unwrap().admission.status = status;
            refuses(&collection, &catalog);
        }
        let (collection, mut catalog) = support::fixture();
        catalog.0.remove(id);
        refuses(&collection, &catalog);
        let (collection, mut catalog) = support::fixture();
        catalog.0.get_mut(id).unwrap().bytes.push(b' ');
        refuses(&collection, &catalog);
    }
    let (mut collection, mut catalog) = support::fixture();
    let mut policy = support::value(&catalog, "policy");
    policy["visibility"] = json!("private");
    support::put(&mut catalog, "policy", &policy);
    support::bind(&mut collection, &catalog);
    refuses(&collection, &catalog);
}

#[test]
fn n03_policy_selection_and_finite_budget_refusals() {
    for (pointer, bad) in [
        ("/registries", json!([])),
        (
            "/sources/0/acquisition_profile/digest",
            json!("a".repeat(64)),
        ),
        (
            "/sources/0/selected_profiles/0/digest",
            json!("a".repeat(64)),
        ),
        ("/sources/0/selectors/0/source_id", json!("other")),
        ("/sources/0/selectors/0/origin", json!("missing")),
        ("/sources/0/limits/requests", json!(0)),
        ("/sources/0/limits/requests", json!(-1)),
        ("/sources/0/limits/requests", json!(1.5)),
        (
            "/sources/0/seeds/0",
            json!("https://user:secret@garden.example/docs"),
        ),
        (
            "/sources/0/seeds/0",
            json!("https://garden.example/docs%2fprivate"),
        ),
        (
            "/sources/0/seeds/0",
            json!("https://garden.example/docs-neighbour"),
        ),
        ("/sources/0/seeds/0", json!("http://garden.example/docs")),
        ("/sources/0/sync/mode", json!("watch")),
        ("/sources/0/id", json!("../notes")),
        ("/sources/0/identity/meaningful_queries", json!(["utm"])),
    ] {
        let (mut collection, mut catalog) = support::fixture();
        let mut policy = support::value(&catalog, "policy");
        *policy.pointer_mut(pointer).unwrap() = bad;
        if pointer.ends_with("meaningful_queries") {
            policy["sources"][0]["identity"]["ignored_tracking_queries"] = json!(["utm"]);
        }
        support::put(&mut catalog, "policy", &policy);
        support::bind(&mut collection, &catalog);
        refuses(&collection, &catalog);
    }
    let (mut collection, mut catalog) = support::fixture();
    let mut decisions = support::value(&catalog, "decisions");
    let mut conflicting = decisions["entries"][0].clone();
    conflicting["id"] = json!("conflicting");
    conflicting["action"] = json!("exclude_from_knowledge");
    decisions["entries"]
        .as_array_mut()
        .unwrap()
        .push(conflicting);
    support::put(&mut catalog, "decisions", &decisions);
    support::rebind(&mut collection, &mut catalog);
    refuses(&collection, &catalog);
}

/// A bounded declarative browser readiness record, not executable CSS.
fn readiness() -> Value {
    json!({"document_state":"load",
             "all_of":[{"kind":"element_present",
             "selector":[{"tag":"article",
             "attributes":[]}]}],
             "timeout_ms":1000,
            "poll_interval_ms":10,
            "stable_for_ms":20})
}

#[test]
fn n03_acquisition_profile_refuses_before_launch() {
    for (pointer, bad) in [
        ("/transport", json!("hidden_default")),
        ("/adapter/id", json!("missing")),
        ("/adapter/digest", json!("a".repeat(64))),
        ("/required_capabilities", json!(["unsupported"])),
        ("/readiness", readiness()),
    ] {
        let (mut collection, mut catalog) = support::fixture();
        let mut profile = support::value(&catalog, "http");
        *profile.pointer_mut(pointer).unwrap() = bad;
        support::put(&mut catalog, "http", &profile);
        support::rebind(&mut collection, &mut catalog);
        refuses(&collection, &catalog);
    }
    for bad in [
        Value::Null,
        json!({"document_state":"load",
             "all_of":[{"kind":"script",
             "script":"run()"}],
             "timeout_ms":1000,
            "poll_interval_ms":1,
            "stable_for_ms":1}),
        json!({"document_state":"load",
             "all_of":[{"kind":"network_idle"}],
             "timeout_ms":1000,
            "poll_interval_ms":1,
            "stable_for_ms":1}),
    ] {
        let (mut collection, mut catalog) = support::fixture();
        let mut profile = support::value(&catalog, "http");
        profile["transport"] = json!("browser_render");
        profile["readiness"] = bad;
        support::put(&mut catalog, "http", &profile);
        support::rebind(&mut collection, &mut catalog);
        refuses(&collection, &catalog);
    }
    for (pointer, bad) in [
        ("/timeout_ms", json!(0)),
        ("/timeout_ms", json!(120_001)),
        ("/poll_interval_ms", json!(0)),
        ("/poll_interval_ms", json!(1001)),
        ("/stable_for_ms", json!(0)),
        ("/stable_for_ms", json!(1001)),
        ("/all_of", json!([])),
        ("/all_of", json!(vec![readiness()["all_of"][0].clone(); 33])),
        ("/all_of/0/selector", json!([])),
        ("/all_of/0/selector/0/tag", json!("article > script")),
        ("/document_state", json!("network_idle")),
    ] {
        let (mut collection, mut catalog) = support::fixture();
        let mut profile = support::value(&catalog, "http");
        let mut ready = readiness();
        *ready.pointer_mut(pointer).unwrap() = bad;
        profile["transport"] = json!("browser_render");
        profile["readiness"] = ready;
        support::put(&mut catalog, "http", &profile);
        support::rebind(&mut collection, &mut catalog);
        refuses(&collection, &catalog);
    }
}

#[test]
fn n03_profile_switch_and_round_trips_are_deterministic() {
    for transport in ["http", "browser_request", "browser_render"] {
        let (mut collection, mut catalog) = support::fixture();
        let mut profile = support::value(&catalog, "http");
        profile["transport"] = json!(transport);
        if transport == "browser_render" {
            profile["readiness"] = readiness();
        }
        support::put(&mut catalog, "http", &profile);
        support::rebind(&mut collection, &mut catalog);
        let checked = resolve(&collection, &catalog).unwrap();
        let text = serde_json::to_string(checked.policy()).unwrap();
        let decoded = maestro_acquisition::parse_policy(&text).unwrap();
        assert_eq!(text, serde_json::to_string(&decoded).unwrap());
        assert_eq!(checked.acquisition_profiles().len(), 1);
    }
}

#[test]
fn n03_closed_automatic_classes_hold_unknown_change_names() {
    use maestro_acquisition::policy::manifest::AutomaticClass;
    for (change, class) in [
        ("select_profile", AutomaticClass::SelectedProfiles),
        ("set_cleanup", AutomaticClass::Cleanup),
        ("set_s1_chunk_strategy", AutomaticClass::S1ChunkStrategy),
        ("set_dedup_keys", AutomaticClass::DedupKeys),
        (
            "add_knowledge_exclusion",
            AutomaticClass::NewKnowledgeExclusions,
        ),
        ("add_asset_only", AutomaticClass::NewKnowledgeExclusions),
    ] {
        assert_eq!(AutomaticClass::for_change(change), Some(class));
    }
    for unknown in [
        "deny_fetch",
        "promote_knowledge",
        "set_timeout",
        "remove_exclusion",
        "script",
    ] {
        assert_eq!(AutomaticClass::for_change(unknown), None);
    }
}

#[test]
fn n03_nested_objects_null_presence_and_reviewed_platform_are_strict() {
    for (pointer, bad) in [
        ("/schema", json!("maestro-source-policy/2")),
        ("/owner_ref", json!(["owner", "0".repeat(64)])),
        ("/visibility", json!({"public": null})),
        ("/sources/0/robots", json!(["maestro", null, 1000, 1000])),
        (
            "/sources/0/discovery/0",
            json!({"kind":"links", "depth":1, "script":"run"}),
        ),
        (
            "/sources/0/identity/meaningful_queries",
            json!(["x".repeat(4097)]),
        ),
        ("/sources/0/limits/decode/xml_entities", json!("enabled")),
        ("/adaptation/automatic_classes", json!(["grant_access"])),
        (
            "/sources/0/selectors/0",
            json!({"source_id":"notes",
             "origin":null,
            "path_prefix":null,
            "object_ids":[],
            "versions":[],
            "channels":[],
            "media_types":[]}),
        ),
    ] {
        let (mut collection, mut catalog) = support::fixture();
        let mut policy = support::value(&catalog, "policy");
        *policy.pointer_mut(pointer).unwrap() = bad;
        support::put(&mut catalog, "policy", &policy);
        support::bind(&mut collection, &catalog);
        refuses(&collection, &catalog);
    }
    for field in ["auth_role", "connector", "wiki_mapping"] {
        let (mut collection, mut catalog) = support::fixture();
        let mut policy = support::value(&catalog, "policy");
        policy["sources"][0].as_object_mut().unwrap().remove(field);
        support::put(&mut catalog, "policy", &policy);
        support::bind(&mut collection, &catalog);
        refuses(&collection, &catalog);
    }
    for id in ["adapter", "qualification", "http", "decisions"] {
        let (collection, mut catalog) = support::fixture();
        catalog.0.get_mut(id).unwrap().admission.platform = "unqualified-platform".into();
        refuses(&collection, &catalog);
    }
    let (collection, mut catalog) = support::fixture();
    catalog.0.get_mut("adapter").unwrap().admission.capabilities = vec!["fetch".into()];
    refuses(&collection, &catalog);
}

#[test]
fn n03_collection_parser_bounds_precede_typed_values() {
    let (collection, _) = support::fixture();
    let text = collection.to_string();
    assert!(
        format!("{text}{}", " ".repeat(4 * 1024 * 1024))
            .parse::<Declaration>()
            .is_err()
    );
    let deep = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    assert!(
        text.replace("\"Garden notes\"", &deep)
            .parse::<Declaration>()
            .is_err()
    );
    assert!(
        text.replace(
            "\"maestro-collection/2\"",
            "\"maestro-collection/2\",\"schema\":\"maestro-collection/2\""
        )
        .parse::<Declaration>()
        .is_err()
    );
}

#[test]
fn n03_contradictory_overlapping_selectors_refuse() {
    let (mut collection, mut catalog) = support::fixture();
    let mut decisions = support::value(&catalog, "decisions");
    let mut entry = decisions["entries"][0].clone();
    entry["id"] = json!("overlap");
    entry["action"] = json!("exclude_from_knowledge");
    entry["selector"]["path_prefix"] = json!("/docs/start");
    decisions["entries"].as_array_mut().unwrap().push(entry);
    support::put(&mut catalog, "decisions", &decisions);
    support::rebind(&mut collection, &mut catalog);
    refuses(&collection, &catalog);
}

#[test]
fn n03_generated_schema_requires_explicit_nullable_readiness() {
    use maestro_acquisition::policy::acquisition::AcquisitionProfile;
    let schema = serde_json::to_value(schemars::schema_for!(AcquisitionProfile)).unwrap();
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("readiness"))
    );
    assert!(
        schema["properties"]["readiness"]
            .to_string()
            .contains("null")
    );
}

#[test]
fn n03_unselected_profiles_and_discovery_refs_still_validate() {
    let (mut collection, mut catalog) = support::fixture();
    let mut profile = support::value(&catalog, "http");
    profile["id"] = json!("unused");
    profile["transport"] = json!("browser_render");
    let reference = support::put(&mut catalog, "unused", &profile);
    let mut policy = support::value(&catalog, "policy");
    policy["acquisition_profiles"]
        .as_array_mut()
        .unwrap()
        .push(json!(reference));
    support::put(&mut catalog, "policy", &policy);
    support::bind(&mut collection, &catalog);
    refuses(&collection, &catalog);
    let (mut collection, mut catalog) = support::fixture();
    let mut policy = support::value(&catalog, "policy");
    policy["sources"][0]["discovery"] =
        json!([{"kind":"api", "mapping":{"id":"absent","digest":"0".repeat(64)}}]);
    support::put(&mut catalog, "policy", &policy);
    support::bind(&mut collection, &catalog);
    refuses(&collection, &catalog);
}

#[test]
fn n03_parser_ceiling_neighbours() {
    support::parser_boundaries();
}
