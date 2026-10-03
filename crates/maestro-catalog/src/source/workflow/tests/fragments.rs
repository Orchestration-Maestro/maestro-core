//! Checkpoint review regressions for fragment admission and tuple predicates.

use super::contracts::{check, contract_tree};
use crate::{
    limits::Limits,
    source::tests::support::{MemoryTree, check_under},
};
use serde_json::{Value, json};

/// The review's paired contracts, with no declared dependency by default.
fn paired_tree(schema: &Value, declared: bool) -> MemoryTree {
    let metadata = concat!(
        "schema = \"maestro-source/2\"\nmaturity = \"reviewed\"\n",
        "rows = [\"chat.M036 objects\"]\nworkflows = [\"ctm-question\"]\n"
    );
    let target = json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "contract:core/review", "type": "object"});
    contract_tree(&schema.to_string())
        .with("core/contracts/review.schema.json", &target.to_string())
        .with("core/contracts/review.maestro.toml", metadata)
        .with(
            "core/contracts/test-report.maestro.toml",
            &format!(
                "{metadata}{}",
                if declared {
                    "requires = [\"contract:core/review\"]\n"
                } else {
                    ""
                }
            ),
        )
}

#[test]
fn c22b_review_probe_annotation_reference_requires() {
    let mut admitted = Vec::new();
    for (pointer, key, value) in [
        (
            "#/default",
            "default",
            json!({"$ref": "contract:core/review"}),
        ),
        (
            "#/examples/0",
            "examples",
            json!([{"$ref": "contract:core/review"}]),
        ),
    ] {
        let mut schema = json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/test-report", "type": "object", "$ref": pointer});
        schema[key] = value;
        match check_under(&paired_tree(&schema, false), &Limits::PRODUCTION) {
            Ok(_) => admitted.push(pointer),
            Err(refusal) => {
                let refusal = refusal.to_string();
                assert!(
                    refusal.contains(pointer) && refusal.contains("$defs"),
                    "{refusal}"
                );
            }
        }
    }
    assert!(admitted.is_empty(), "admitted data fragments: {admitted:?}");
    let schema = json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "contract:core/test-report", "type": "object", "$ref": "#/$defs/result",
        "$defs": {"result": {"$ref": "contract:core/review"}}});
    assert!(check_under(&paired_tree(&schema, true), &Limits::PRODUCTION).is_ok());
    let refusal = check_under(&paired_tree(&schema, false), &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("requires") && refusal.contains("contract:core/review"),
        "{refusal}"
    );
}

#[test]
fn cross_contract_fragments_keep_declared_reference_and_schema_position_guards() {
    for reference_key in ["$ref", "$dynamicRef"] {
        let mut root = json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/test-report", "type": "object"});
        let target = json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/review", "type": "object",
            "default": {"type": "object"}, "$defs": {"result": {"type": "object"}}});
        for (fragment, admitted) in [("#/default", false), ("#/$defs/result", true)] {
            let reference = format!("contract:core/review{fragment}");
            root[reference_key] = json!(reference);
            let tree = paired_tree(&root, true)
                .with("core/contracts/review.schema.json", &target.to_string());
            let result = check_under(&tree, &Limits::PRODUCTION);
            assert_eq!(result.is_ok(), admitted, "{reference_key}: {result:?}");
            if let Err(refusal) = result {
                let refusal = refusal.to_string();
                assert!(
                    refusal.contains(&reference) && refusal.contains("$defs"),
                    "{refusal}"
                );
            }
            let tree = paired_tree(&root, false)
                .with("core/contracts/review.schema.json", &target.to_string());
            let refusal = check_under(&tree, &Limits::PRODUCTION)
                .unwrap_err()
                .to_string();
            assert!(refusal.contains("requires"), "{refusal}");
        }
    }
}

#[test]
fn c22b_review_probe_tuple_predicate_refuses() {
    let mut schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "contract:core/test-report", "type": "object", "required": ["reviews"],
        "properties": {"reviews": {"type": "array", "prefixItems": [{"type": "integer"}],
            "items": {"type": "object", "required": ["verdict"],
                "properties": {"verdict": {"type": "string"}}}}}
    });
    assert!(check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION).is_ok());
    let registry = jsonschema::Registry::new().prepare().unwrap();
    let validator = super::super::contracts::validator(&schema, &registry).unwrap();
    assert!(validator.validate(&json!({"reviews": [1]})).is_ok());
    let mut admitted = Vec::new();
    for predicate in ["all", "any"] {
        let text = format!("{predicate}(reviews, r => r.verdict == 'approved')");
        match check(&text, &schema) {
            Ok(()) => admitted.push(predicate),
            Err(refusal) => assert!(
                refusal.contains("prefixItems") && refusal.contains("items"),
                "{refusal}"
            ),
        }
    }
    assert!(
        admitted.is_empty(),
        "admitted tuple predicates: {admitted:?}"
    );
    schema["properties"]["reviews"]
        .as_object_mut()
        .unwrap()
        .remove("prefixItems");
    for predicate in ["all", "any"] {
        let text = format!("{predicate}(reviews, r => r.verdict == 'approved')");
        assert_eq!(check(&text, &schema), Ok(()), "{text}");
    }
}
