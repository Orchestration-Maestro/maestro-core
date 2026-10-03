//! Native representation and registry refusals before schema compilation.

use super::super::contracts;
use super::{
    contracts::contract_tree,
    support::{id, resource},
};
use crate::{
    limits::Limits,
    source::{
        load::{Loaded, Native},
        tests::support::check_under,
    },
};
use serde_json::{Value, json};
use std::slice::from_ref;

/// One checked contract with an explicitly supplied native representation.
fn loaded(native: Native) -> Loaded {
    Loaded {
        resource: resource(
            id("contract:core/test-report"),
            "core/contracts/test-report.schema.json",
        ),
        native,
        metadata_path: "core/contracts/test-report.maestro.toml".to_owned(),
        prefix: String::new(),
    }
}

/// A valid primary schema identity, reused by each refusal neighbour.
fn schema() -> Value {
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "contract:core/test-report", "type": "object"})
}

#[test]
fn contract_native_input_must_be_json_not_missing_or_cedar() {
    assert!(contracts::check(&[loaded(Native::Json(schema()))]).is_empty());
    for (native, message) in [
        (Native::None, "contract needs native JSON input"),
        (
            Native::Cedar("permit();".to_owned()),
            "contract needs JSON, not Cedar (9 bytes)",
        ),
    ] {
        let diagnostics = contracts::check(&[loaded(native)]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].key, "contract");
        assert_eq!(
            diagnostics[0].path,
            "core/contracts/test-report.schema.json"
        );
        assert_eq!(diagnostics[0].message, message);
    }
}

#[test]
fn contract_declared_reference_still_needs_a_checked_snapshot_member() {
    let mut root_schema = schema();
    root_schema["$ref"] = json!("contract:core/review");
    let mut root = loaded(Native::Json(root_schema));
    root.resource
        .metadata
        .requires
        .push(id("contract:core/review"));
    let diagnostics = contracts::check(from_ref(&root));
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("contract:core/review is missing from the checked snapshot")
    );
    let mut target_schema = schema();
    target_schema["$id"] = json!("contract:core/review");
    let mut target = loaded(Native::Json(target_schema));
    target.resource = resource(
        id("contract:core/review"),
        "core/contracts/review.schema.json",
    );
    assert!(contracts::check(&[root, target]).is_empty());
}

#[test]
fn contract_data_fragment_invalid_uri_reports_registry_failure() {
    let mut root = schema();
    root["$ref"] = json!("#/default");
    root["default"] = json!({"$id": "https://[invalid", "type": "object"});
    let refusal = check_under(&contract_tree(&root.to_string()), &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("core/contracts/test-report.schema.json"),
        "{refusal}"
    );
    assert!(refusal.contains("invalid"), "{refusal}");
    root["$ref"] = json!("#/$defs/result");
    root["$defs"] = json!({"result": {"type": "object"}});
    // The invalid URI remains inert instance data when it is not a reference target.
    assert!(check_under(&contract_tree(&root.to_string()), &Limits::PRODUCTION).is_ok());
}
