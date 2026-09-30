//! Four ruled N03 parser and schema regressions.
use super::support;
use maestro_acquisition::{
    PolicySource,
    policy::{
        acquisition::{AcquisitionProfile, Attribute, DomStep, Readiness},
        decisions::{Decision, Decisions, Promotion, Promotions},
        limits::{DecodeLimits, Limits},
        manifest::{AcquisitionManifest, AdaptationPolicy},
        resolve::parse_resource,
        schema::SourcePolicy,
        source::{IdentityRule, Origin, Robots, Selector, Source, SyncPolicy},
        wiki::WikiMapping,
    },
};
use maestro_knowledge::collection::Declaration;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[test]
fn n03_review_v1_string_semantics_are_preserved() {
    for (pointer, text) in [
        ("/title", "a".repeat(8193)),
        ("/title", "contains\0nul".into()),
        ("/profiles/embedding", "a".repeat(8193)),
    ] {
        for version in [1, 2] {
            let (mut collection, _) = support::fixture();
            collection["schema"] = json!(format!("maestro-collection/{version}"));
            if version == 1 {
                collection.as_object_mut().unwrap().remove("source_policy");
            }
            *collection.pointer_mut(pointer).unwrap() = json!(text);
            let result = collection.to_string().parse::<Declaration>();
            assert_eq!(
                result.is_ok(),
                version == 1,
                "v{version} {pointer}: {result:?}"
            );
        }
    }
}

#[test]
fn n03_review_ambiguous_unused_origin_refuses() {
    for host in [
        "127.1",
        "2130706433",
        "0x7f.1",
        "127.0.0.1",
        "[::1]",
        "Garden.example",
        "garden.example.",
    ] {
        let (mut collection, mut catalog) = support::fixture();
        let mut policy = support::value(&catalog, "policy");
        let mut origin = policy["sources"][0]["origins"][0].clone();
        origin["id"] = json!("ambiguous");
        origin["host"] = json!(host);
        origin["purpose"] = json!("authentication");
        policy["sources"][0]["origins"]
            .as_array_mut()
            .unwrap()
            .push(origin);
        support::put(&mut catalog, "policy", &policy);
        support::bind(&mut collection, &catalog);
        let declaration = collection.to_string().parse::<Declaration>().unwrap();
        let scopes = support::scopes();
        let result = catalog.resolve(&declaration, &support::principal(&scopes));
        assert!(result.is_err(), "unused origin {host} admitted: {result:?}");
    }
}

/// Independently authored mapping; execution remains deferred to N47.
fn wiki() -> Value {
    json!({"schema":"maestro-wiki-mapping/1", "id":"mapping", "version":1,
        "collection_id":"garden", "visibility":"public",
        "scope_tags":["workspace/default/collection/garden"],
        "owner_ref":{"id":"owner","digest":"0".repeat(64)},
        "origin_id":"content", "list_endpoint":"/docs/list", "item_endpoint":"/docs/item",
        "items_path":[], "identity_path":[], "parent_path":[], "revision_path":[],
        "content_path":[], "content_kind":"markdown", "block_mapping":null,
        "attachments_path":[], "permissions_path":[], "permission_semantics":"explicit_scopes",
        "pagination":{"kind":"next_link","response_path":[],"terminal_path":[]},
        "withdrawal":"complete_inventory", "tombstone_path":null})
}

#[test]
fn n03_review_every_wiki_field_path_is_bounded() {
    for kind in ["cursor", "next_link"] {
        let mut baseline = wiki();
        baseline["pagination"]["kind"] = json!(kind);
        if kind == "cursor" {
            baseline["pagination"]["request_field"] = json!("cursor");
        }
        for pointer in [
            "/items_path",
            "/identity_path",
            "/parent_path",
            "/revision_path",
            "/content_path",
            "/attachments_path",
            "/permissions_path",
            "/tombstone_path",
            "/pagination/response_path",
            "/pagination/terminal_path",
        ] {
            for steps in [32, 33] {
                let mut mapping = baseline.clone();
                *mapping.pointer_mut(pointer).unwrap() =
                    json!(vec![json!({"kind":"field","name":"items"}); steps]);
                let result = parse_resource::<WikiMapping>(&serde_json::to_vec(&mapping).unwrap());
                assert_eq!(
                    result.is_ok(),
                    steps == 32,
                    "{kind} {pointer}: {steps} steps admitted incorrectly"
                );
            }
        }
    }
}

#[test]
fn n03_review_wiki_field_path_schema_has_max_items() {
    let schema = serde_json::to_value(schemars::schema_for!(WikiMapping)).unwrap();
    assert_eq!(schema["$defs"]["FieldPath"]["maxItems"], json!(32));
    assert_eq!(check_path_schemas(&schema), 12);
}

/// Follow the schema tree so inline, referenced and nullable paths are checked.
fn check_path_schemas(schema: &Value) -> usize {
    match schema {
        Value::Object(fields) => fields
            .iter()
            .map(|(name, value)| {
                let path = name.ends_with("_path");
                if path {
                    assert!(
                        value.to_string().contains("#/$defs/FieldPath"),
                        "{name}: {value}"
                    );
                }
                usize::from(path) + check_path_schemas(value)
            })
            .sum(),
        Value::Array(items) => items.iter().map(check_path_schemas).sum(),
        _ => 0,
    }
}

/// Compare schema-required fields with real omission refusals, including nullables.
fn required_fields<T: DeserializeOwned + JsonSchema>(value: &Value) {
    assert!(parse_resource::<T>(&serde_json::to_vec(value).unwrap()).is_ok());
    let schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    let required: BTreeSet<_> = schema
        .get("required")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap().to_owned())
        .collect();
    let mut parser_required = BTreeSet::new();
    for field in value.as_object().unwrap().keys() {
        let mut omitted = value.clone();
        omitted.as_object_mut().unwrap().remove(field);
        if parse_resource::<T>(&serde_json::to_vec(&omitted).unwrap()).is_err() {
            parser_required.insert(field.clone());
        }
    }
    assert_eq!(
        required,
        parser_required,
        "{} required fields disagree",
        T::schema_name()
    );
}

#[test]
fn n03_review_resource_schema_required_fields_match_parser() {
    let (_, catalog) = support::fixture();
    let policy = support::value(&catalog, "policy");
    let source = &policy["sources"][0];
    let decisions = support::value(&catalog, "decisions");
    let profile = support::value(&catalog, "http");
    required_fields::<Robots>(&source["robots"]);
    required_fields::<SourcePolicy>(&policy);
    required_fields::<Source>(source);
    required_fields::<Origin>(&source["origins"][0]);
    required_fields::<Selector>(&source["selectors"][0]);
    required_fields::<IdentityRule>(&source["identity"]);
    required_fields::<SyncPolicy>(&source["sync"]);
    required_fields::<Limits>(&source["limits"]);
    required_fields::<DecodeLimits>(&source["limits"]["decode"]);
    required_fields::<AdaptationPolicy>(&policy["adaptation"]);
    required_fields::<Decisions>(&decisions);
    required_fields::<Decision>(&decisions["entries"][0]);
    required_fields::<AcquisitionProfile>(&profile);
    required_fields::<WikiMapping>(&wiki());
    let mut promotions = decisions.clone();
    promotions["schema"] = json!("maestro-source-promotions/1");
    promotions["entries"][0]["action"] = json!("promote_knowledge");
    required_fields::<Promotions>(&promotions);
    required_fields::<Promotion>(&promotions["entries"][0]);
    let mut manifest = profile;
    for field in [
        "transport",
        "adapter",
        "required_capabilities",
        "readiness",
        "qualification",
    ] {
        manifest.as_object_mut().unwrap().remove(field);
    }
    let reference = json!({"id":"baseline","digest":"0".repeat(64)});
    manifest["schema"] = json!("maestro-acquisition-manifest/1");
    manifest["baseline"] = reference.clone();
    manifest["baseline_kind"] = json!("local");
    manifest["proposals"] = json!([]);
    manifest["activations"] = json!([]);
    manifest["active"] = reference;
    manifest["effective_digest"] = json!("0".repeat(64));
    required_fields::<AcquisitionManifest>(&manifest);
    let ready = json!({"document_state":"load","all_of":[],
        "timeout_ms":1,"poll_interval_ms":1,"stable_for_ms":1});
    required_fields::<Readiness>(&ready);
    required_fields::<DomStep>(&json!({"tag":"article","attributes":[]}));
    required_fields::<Attribute>(&json!({"name":"id","value":"content"}));
}
