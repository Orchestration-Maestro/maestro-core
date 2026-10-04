//! Fragment targets are schemas only at positions defined by the admitted draft.

use super::contracts::contract_tree;
use crate::{limits::Limits, source::tests::support::check_under};
use serde_json::json;

#[test]
fn fragment_targets_refuse_data_keywords_and_data_only_anchors() {
    for reference_key in ["$ref", "$dynamicRef"] {
        for (key, value, fragment) in [
            ("default", json!({"type": "object"}), "#/default"),
            ("examples", json!([{"type": "object"}]), "#/examples/0"),
            ("const", json!({"type": "object"}), "#/const"),
            ("enum", json!([{"type": "object"}]), "#/enum/0"),
            ("unknown", json!({"type": "object"}), "#/unknown"),
            (
                "default",
                json!({"$anchor": "hidden", "type": "object"}),
                "#hidden",
            ),
            (
                "default",
                json!({"$dynamicAnchor": "hidden", "type": "object"}),
                "#hidden",
            ),
            ("default", json!({"type": "object"}), "#/def%61ult"),
        ] {
            let mut schema = json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
                "$id": "contract:core/test-report", "type": "object"});
            schema[key] = value;
            // Inert data still admits before a schema-position reference promotes it.
            assert!(check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION).is_ok());
            schema[reference_key] = json!(fragment);
            let refusal = check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION)
                .unwrap_err()
                .to_string();
            assert!(
                refusal.contains(fragment) && refusal.contains("$defs"),
                "{refusal}"
            );
        }
    }
}

#[test]
fn fragment_targets_admit_draft_subschemas() {
    for (container, fragment) in [
        (
            json!({"$defs": {"result": {"type": "object"}}}),
            "#/$defs/result",
        ),
        (
            json!({"properties": {"result": {"type": "object"}}}),
            "#/properties/result",
        ),
        (json!({"items": {"type": "object"}}), "#/items"),
        (
            json!({"prefixItems": [{"type": "object"}]}),
            "#/prefixItems/0",
        ),
        (json!({"allOf": [{"type": "object"}]}), "#/allOf/0"),
        (
            json!({"$defs": {"a/b~c": {"type": "object"}}}),
            "#/$defs/a~1b~0c",
        ),
        (
            json!({"$defs": {"result": {"$anchor": "result", "type": "object"}}}),
            "#result",
        ),
        (
            json!({"$defs": {"result": {"$dynamicAnchor": "result", "type": "object"}}}),
            "#result",
        ),
    ] {
        for reference_key in ["$ref", "$dynamicRef"] {
            let mut schema = container.clone();
            schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
            schema["$id"] = json!("contract:core/test-report");
            schema["type"] = json!("object");
            schema[reference_key] = json!(fragment);
            assert!(
                check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION).is_ok(),
                "{schema}"
            );
        }
    }
}
