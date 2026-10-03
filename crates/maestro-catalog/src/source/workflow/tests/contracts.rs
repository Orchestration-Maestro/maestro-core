//! Contract conditions: typed neighbours and hostile text never execute.

use super::super::{conditions, contracts::validator};
use crate::{
    limits::Limits,
    source::{
        Entry, Known, SourceTree, builtin, check as check_source, frozen_rows,
        tests::support::{MemoryTree, check_under},
    },
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, io};

#[test]
fn incomplete_graph_kinds_are_not_admitted_at_the_checkpoint() {
    let registry = builtin().unwrap();
    let kinds: Vec<&str> = registry
        .registrations()
        .map(|registration| registration.descriptor.kind.as_str())
        .collect();
    // R10: workflow stays absent until flat source/2 and all graph rules are checked.
    assert!(!kinds.contains(&"workflow"));
    // R8: policy stays absent until real Cedar validation and coverage are checked.
    assert!(!kinds.contains(&"policy"));
    // R3: session-profile stays absent until exact checked bindings are supported.
    assert!(!kinds.contains(&"session-profile"));
    // R8: policy-schema stays absent until the shared Cedar schema is checked.
    assert!(!kinds.contains(&"policy-schema"));
    // R8: policy-case stays absent until checked allow/deny neighbours are supported.
    assert!(!kinds.contains(&"policy-case"));
}

/// A source that changes its contract on a second original read.
#[derive(Debug)]
struct MutatingTree {
    /// Original checked source tree.
    tree: MemoryTree,
    /// Original reads, not accesses to the retained snapshot.
    reads: RefCell<BTreeMap<String, usize>>,
}

impl SourceTree for MutatingTree {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        self.tree.list(directory)
    }
    fn read(&self, file: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        let count = {
            let mut reads = self.reads.borrow_mut();
            let count = reads.entry(file.to_owned()).or_default();
            *count += 1;
            *count
        };
        if file == "core/contracts/test-report.schema.json" && count > 1 {
            return Ok(b"{\"type\":\"string\"}".to_vec());
        }
        self.tree.read(file, max_bytes)
    }
}

#[test]
fn native_contract_semantics_use_one_original_snapshot_read() {
    let tree = MutatingTree {
        tree: contract_tree(&schema().to_string()),
        reads: RefCell::default(),
    };
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let catalog = check_source(
        &tree,
        &builtin().unwrap(),
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: 20_727,
        },
    )
    .unwrap();
    assert!(
        catalog
            .resources
            .iter()
            .any(|resource| resource.id.kind == "contract")
    );
    let reads = tree.reads.borrow();
    assert_eq!(reads.len(), 10);
    assert!(reads.values().all(|count| *count == 1), "{reads:?}");
}

/// A checked in-memory schema registry, shared with the condition resolver.
pub(super) fn check(text: &str, schema: &Value) -> Result<(), String> {
    let mut schema = schema.clone();
    if schema.get("$id").is_none() {
        schema["$id"] = json!("contract:core/test-report");
    }
    let id = schema["$id"].as_str().unwrap();
    let registry = jsonschema::Registry::new()
        .draft(jsonschema::Draft::Draft202012)
        .add(id, &schema)
        .unwrap()
        .prepare()
        .map_err(|error| error.to_string())?;
    conditions::check(text, &schema, &registry)
}

/// A native contract paired with the existing strict metadata envelope.
pub(super) fn contract_tree(text: &str) -> MemoryTree {
    MemoryTree::valid()
        .with("core/contracts/test-report.schema.json", text)
        .with(
            "core/contracts/test-report.maestro.toml",
            concat!(
                "schema = \"maestro-source/2\"\nmaturity = \"reviewed\"\n",
                "rows = [\"chat.M036 objects\"]\nworkflows = [\"ctm-question\"]\n"
            ),
        )
}

#[test]
fn native_contract_loader_retains_json_and_refuses_duplicate_keys() {
    let schema = schema().to_string();
    let catalog = check_under(&contract_tree(&schema), &Limits::PRODUCTION).unwrap();
    assert!(
        catalog
            .resources
            .iter()
            .any(|resource| resource.id.kind == "contract")
    );
    for text in [
        "{\"type\":\"object\",\"type\":\"string\"}",
        "{\"properties\":{\"x\":{\"type\":\"integer\",\"type\":\"string\"}}}",
    ] {
        let refusal = check_under(&contract_tree(text), &Limits::PRODUCTION)
            .unwrap_err()
            .to_string();
        assert!(
            refusal.contains("core/contracts/test-report.schema.json"),
            "{refusal}"
        );
        assert!(refusal.contains("duplicate object key"), "{refusal}");
    }
}

/// A source-node outcome containing scalars, objects and arrays.
fn schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "contract:core/test-report",
        "type": "object",
        "required": ["tests", "approved", "reviews"],
        "properties": {
            "tests": {"type": "object", "required": ["failed", "executed"], "properties": {
                "failed": {"type": "integer"}, "executed": {"type": "integer"}
            }},
            "approved": {"type": "boolean"},
            "reviews": {"type": "array", "items": {
                "type": "object", "required": ["verdict"],
                "properties": {"verdict": {"type": "string"}}
            }}
        }
    })
}

#[test]
fn contract_identity_and_declared_local_references_are_exact() {
    let reference = "contract:core/review#/$defs/result";
    let mut root = schema();
    root["properties"]["review"] = json!({"$ref": reference});
    let target = json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "contract:core/review", "type": "object", "$defs": {"result": {"type": "string"}}});
    let metadata = concat!(
        "schema = \"maestro-source/2\"\nmaturity = \"reviewed\"\n",
        "rows = [\"chat.M036 objects\"]\nworkflows = [\"ctm-question\"]\n"
    );
    let tree = contract_tree(&root.to_string())
        .with(
            "core/contracts/test-report.maestro.toml",
            &format!("{metadata}requires = [\"contract:core/review\"]\n"),
        )
        .with("core/contracts/review.schema.json", &target.to_string())
        .with("core/contracts/review.maestro.toml", metadata);
    assert!(check_under(&tree, &Limits::PRODUCTION).is_ok());
    let undeclared = tree
        .clone()
        .with("core/contracts/test-report.maestro.toml", metadata);
    let refusal = check_under(&undeclared, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("contract:core/review") && refusal.contains("requires"),
        "{refusal}"
    );
    for reference in [
        "review",
        "contracts/review.schema.json",
        "https://example.invalid/review",
        "contract:core/review#/$defs/missing",
    ] {
        root["properties"]["review"]["$ref"] = json!(reference);
        let refusal = check_under(
            &tree
                .clone()
                .with("core/contracts/test-report.schema.json", &root.to_string()),
            &Limits::PRODUCTION,
        )
        .unwrap_err()
        .to_string();
        assert!(
            refusal.contains("core/contracts/test-report.schema.json"),
            "{refusal}"
        );
    }
    for wrong_id in [
        None,
        Some("test-report"),
        Some("https://example.invalid/test-report"),
        Some("contract:core/review"),
    ] {
        let mut root = schema();
        if let Some(wrong_id) = wrong_id {
            root["$id"] = json!(wrong_id);
        } else {
            root.as_object_mut().unwrap().remove("$id");
        }
        assert!(check_under(&contract_tree(&root.to_string()), &Limits::PRODUCTION).is_err());
    }
}

#[test]
fn subschema_ids_refuse_without_reinterpreting_instance_data() {
    for nested_id in ["#alias", "contract:core/review"] {
        let mut schema = schema();
        schema["$defs"] = json!({"nested": {"$id": nested_id, "type": "string"}});
        let target = json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/review", "type": "object"});
        let tree = contract_tree(&schema.to_string())
            .with("core/contracts/review.schema.json", &target.to_string())
            .with(
                "core/contracts/review.maestro.toml",
                concat!(
                    "schema = \"maestro-source/2\"\nmaturity = \"reviewed\"\n",
                    "rows = [\"chat.M036 objects\"]\nworkflows = [\"ctm-question\"]\n"
                ),
            );
        let refusal = check_under(&tree, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string();
        assert!(refusal.contains("/$defs/nested/$id"), "{refusal}");
        assert!(refusal.contains("$anchor"), "{refusal}");
    }
    for (key, value) in [
        ("default", json!({"$id": "contract:core/review"})),
        ("examples", json!([{"$id": "contract:core/review"}])),
        ("const", json!({"$id": "contract:core/review"})),
        ("enum", json!([{"$id": "contract:core/review"}])),
    ] {
        let mut schema = schema();
        schema[key] = value;
        assert!(
            check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION).is_ok(),
            "{key}"
        );
    }
    let mut schema = schema();
    schema["$defs"] = json!({"flag": {"$anchor": "flag", "type": "boolean"}});
    schema["properties"]["approved"] = json!({"$ref": "#flag"});
    assert!(check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION).is_ok());
    assert_eq!(check("approved", &schema), Ok(()));
}

#[test]
fn native_contract_depth_limit_has_an_exact_valid_neighbour() {
    let depth = Limits::PRODUCTION.source_depth;
    for (arrays, accepted) in [(depth - 1, true), (depth, false)] {
        let schema = format!(
            "{{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\
             \"$id\":\"contract:core/test-report\",\"type\":\"object\",\"default\":{}0{}}}",
            "[".repeat(arrays),
            "]".repeat(arrays),
        );
        let result = check_under(&contract_tree(&schema), &Limits::PRODUCTION);
        assert_eq!(result.is_ok(), accepted, "{arrays}: {result:?}");
        if let Err(refusal) = result {
            let refusal = refusal.to_string();
            assert!(
                refusal.contains("core/contracts/test-report.schema.json"),
                "{refusal}"
            );
            assert!(refusal.contains("JSON depth exceeds"), "{refusal}");
            assert!(!refusal.contains("policy"), "{refusal}");
        }
    }
}

#[test]
fn catalog_check_runs_real_native_contract_validation() {
    for schema in [
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/test-report", "type": "object", "required": 3}),
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/test-report", "type": "object",
            "properties": {"x": {"type": "invented"}}}),
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/test-report", "type": "object",
            "$ref": "file:///never-read.schema.json"}),
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/test-report", "type": "object",
            "$ref": "https://example.invalid/never-fetch.schema.json"}),
    ] {
        let refusal = check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION)
            .unwrap_err()
            .to_string();
        assert!(
            refusal.contains("core/contracts/test-report.schema.json"),
            "{refusal}"
        );
    }
}

#[test]
fn jsonschema_validator_reuses_the_checked_schema() {
    let mut schema = schema();
    schema["required"] = json!(["approved"]);
    schema["additionalProperties"] = json!(false);
    let registry = jsonschema::Registry::new().prepare().unwrap();
    let contract = validator(&schema, &registry).unwrap();
    for value in [json!({"approved": true}), json!({"approved": false})] {
        assert!(contract.validate(&value).is_ok());
    }
    for value in [
        json!({}),
        json!({"approved": "true"}),
        json!({"approved": true, "unknown": 1}),
    ] {
        assert!(contract.validate(&value).is_err(), "accepted {value}");
    }
}

#[test]
fn jsonschema_contract_shape_and_remote_references_refuse() {
    for schema in [
        json!({"$id": "contract:core/test-report", "type": "object"}),
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/test-report", "type": "string"}),
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/test-report", "type": "object", "properties": 3}),
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "contract:core/test-report", "type": "object",
            "$ref": "https://example.invalid/never-fetch"}),
    ] {
        assert!(check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION).is_err());
    }
}

#[test]
fn unused_schema_references_refuse_but_instance_reference_text_is_inert() {
    let mut unused = schema();
    unused["$defs"] = json!({"unused": {"$ref": "https://example.invalid/never-fetch"}});
    assert!(check_under(&contract_tree(&unused.to_string()), &Limits::PRODUCTION).is_err());
    let mut inert = schema();
    inert["default"] = json!({"$ref": "https://example.invalid/inert-instance"});
    assert!(check_under(&contract_tree(&inert.to_string()), &Limits::PRODUCTION).is_ok());
}

#[test]
fn condition_types_accept_valid_neighbours() {
    let schema = schema();
    for text in [
        "tests.failed > 0",
        "tests.failed == 0 && tests.executed > 0",
        "!approved || (tests.failed <= 0 && tests.executed >= 1)",
        "any(reviews, r => r.verdict == 'changes_requested')",
        "all(reviews, r => r.verdict == 'approved')",
        "approved != false",
    ] {
        assert_eq!(check(text, &schema), Ok(()), "{text}");
    }
}

#[test]
fn condition_limits_and_local_references_have_boundary_neighbours() {
    let schema = schema();
    let limit = usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap();
    let text = format!("{}true", " ".repeat(limit - 4));
    assert_eq!(check(&text, &schema), Ok(()));
    assert!(check(&format!("{text} "), &schema).is_err());
    let limit = Limits::PRODUCTION.source_depth;
    let valid = format!("{}true{}", "(".repeat(limit - 1), ")".repeat(limit - 1));
    assert_eq!(check(&valid, &schema), Ok(()));
    assert!(check(&format!("({valid})"), &schema).is_err());
    let schema = json!({"type": "object", "required": ["count"],
        "$defs": {"count": {"type": "integer"}},
        "properties": {"count": {"$ref": "#/$defs/count"}}});
    assert_eq!(check("count >= -1e2", &schema), Ok(()));
    let schema = json!({"type": "object", "required": ["count"],
        "$defs": {"count": {"$ref": "#/$defs/count"}},
        "properties": {"count": {"$ref": "#/$defs/count"}}});
    assert!(check("count >= 0", &schema).is_err());
}

#[test]
fn condition_reference_fields_keep_their_checked_resource_scope() {
    let root = json!({"$id": "contract:core/test-report", "type": "object", "required": ["review"],
        "properties": {"review": {"$ref": "contract:core/review#/$defs/result"}}});
    let target = json!({"$id": "contract:core/review", "type": "object", "$defs": {
        "verdict": {"$anchor": "verdict", "type": "string"},
        "result": {"type": "object", "required": ["verdict"],
            "properties": {"verdict": {"$ref": "#verdict"}}}
    }});
    let registry = jsonschema::Registry::new()
        .draft(jsonschema::Draft::Draft202012)
        .add("contract:core/test-report", &root)
        .unwrap()
        .add("contract:core/review", &target)
        .unwrap()
        .prepare()
        .unwrap();
    assert_eq!(
        conditions::check("review.verdict == 'approved'", &root, &registry),
        Ok(())
    );
    assert!(conditions::check("review.verdict > 0", &root, &registry).is_err());
}

#[test]
fn condition_optional_fields_and_string_ordering_refuse() {
    let mut optional = schema();
    optional["required"] = json!(["tests", "reviews"]);
    assert!(check("approved", &optional).is_err());
    optional = schema();
    optional["properties"]["tests"]["required"] = json!(["executed"]);
    assert!(check("tests.failed == 0", &optional).is_err());
    optional = schema();
    optional["properties"]["reviews"]["items"]["required"] = json!([]);
    assert!(check("all(reviews, r => r.verdict == 'approved')", &optional).is_err());
    for text in ["'a' < 'b'", "'a' <= 'b'", "'a' > 'b'", "'a' >= 'b'"] {
        assert!(check(text, &schema()).is_err(), "accepted {text}");
    }
}

#[test]
fn nested_array_predicates_preserve_lexical_scope() {
    let schema = json!({"type": "object", "required": ["scores"], "properties": {"scores": {
        "type": "array", "items": {"type": "array", "items": {"type": "number"}}
    }}});
    assert_eq!(
        check("all(scores, row => any(row, score => score > 0))", &schema),
        Ok(())
    );
    assert!(
        check(
            "any(scores, row => any(row, score => score > 0)) && score > 0",
            &schema
        )
        .is_err()
    );
    for text in [
        "true ==",
        "true == true == true",
        "'x' < 1",
        "true < false",
        "null > null",
        "scores == scores",
        "all(scores, row.value => true)",
        "all(scores, true => true)",
        "all(scores, row => true, false)",
        "'unterminated",
        "'bad\\q' == 'bad'",
        "'bad\\u12xx' == 'bad'",
        "'bad\ncontrol' == 'bad'",
        "-1e999 > 0",
        "true || (false &&)",
    ] {
        assert!(check(text, &schema).is_err(), "accepted {text}");
    }
}

#[test]
fn condition_types_and_hostile_text_refuse() {
    let schema = schema();
    for text in [
        "tests.failed > 'zero'",
        "tests.absent == 0",
        "approved > 0",
        "tests.failed",
        "!tests.failed",
        "approved && 1",
        "1 || approved",
        "any(tests, r => r.failed == 0)",
        "all(reviews, r => r.verdict)",
        "any(reviews, r => missing.verdict == 'approved')",
        "approved()",
        "system('touch marker')",
        "tests.failed = 0",
        "tests.failed + 1 > 0",
        "approved; shell('echo hostile')",
        "",
        "approved &&",
        "(approved",
        "any(reviews, r => r.verdict == 'approved'",
        "true false",
    ] {
        assert!(check(text, &schema).is_err(), "accepted {text}");
    }
}
