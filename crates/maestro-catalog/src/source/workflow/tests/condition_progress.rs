//! Lexer boundaries and parser depth accounting use advancing valid neighbours.

use super::contracts::{check, contract_tree};
use crate::limits::Limits;
use crate::source::tests::support::check_under;
use serde_json::json;

#[test]
fn condition_identifier_boundaries_have_declared_valid_and_invalid_neighbours() {
    let schema = json!({"type": "object",
        "required": ["_", "-", "_a", "-a", "a_", "a-", "a1", "1a"],
        "properties": {"_": {"type": "boolean"}, "-": {"type": "boolean"},
            "_a": {"type": "boolean"}, "-a": {"type": "boolean"},
            "a_": {"type": "boolean"}, "a-": {"type": "boolean"},
            "a1": {"type": "boolean"}, "1a": {"type": "boolean"}}});
    for (valid, invalid) in [("_", "-"), ("_a", "-a"), ("a_", "a-"), ("a1", "1a")] {
        assert_eq!(check(valid, &schema), Ok(()), "{valid}");
        assert_eq!(
            check(invalid, &schema),
            Err(format!("unsupported condition token {invalid:?}")),
            "{invalid}"
        );
    }
}

#[test]
fn condition_zero_width_tokens_refuse_at_the_lexer_boundary() {
    let schema = json!({"type": "object"});
    for text in [";", "true;", "true && ;", " true || ;"] {
        assert_eq!(
            check(text, &schema),
            Err("unsupported condition token \"\"".to_owned()),
            "{text}"
        );
    }
    assert_eq!(check(" 'a' == \"b\" ", &schema), Ok(()));
    assert_eq!(check(" true && false || true ", &schema), Ok(()));
    assert_eq!(check("true   ", &schema), Ok(()));
}

#[test]
fn workflow_contract_empty_reference_is_not_a_local_fragment() {
    let mut schema = json!({"$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "contract:core/test-report", "type": "object",
        "$defs": {"flag": {"type": "boolean"}}, "properties": {"flag": {"$ref": ""}}});
    for reference in ["#/$defs/flag", "contract:core/test-report#/$defs/flag"] {
        schema["properties"]["flag"]["$ref"] = json!(reference);
        assert!(check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION).is_ok());
    }
    schema["properties"]["flag"]["$ref"] = json!("");
    let refusal = check_under(&contract_tree(&schema.to_string()), &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("needs a fragment or qualified contract ID"),
        "{refusal}"
    );
}

#[test]
fn condition_depth_is_restored_between_sibling_operands() {
    let schema = json!({"type": "object"});
    let depth = Limits::PRODUCTION.source_depth;
    let grouped = format!("{}true{}", "(".repeat(depth - 1), ")".repeat(depth - 1));
    let negated = format!("{}true", "!".repeat(depth - 1));
    for operator in ["&&", "||", "=="] {
        assert_eq!(
            check(&format!("{grouped} {operator} {grouped}"), &schema),
            Ok(())
        );
        assert_eq!(
            check(&format!("{negated} {operator} {negated}"), &schema),
            Ok(())
        );
    }
    assert_eq!(
        check(&vec!["true"; depth + 1].join(" && "), &schema),
        Ok(())
    );
    for too_deep in [format!("({grouped})"), format!("!{negated}")] {
        assert_eq!(
            check(&too_deep, &schema),
            Err("condition nesting exceeds 32 levels".to_owned())
        );
    }
}
