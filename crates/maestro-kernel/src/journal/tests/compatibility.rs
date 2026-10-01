//! The compatibility check's rules, on small schemas: what it refuses as a
//! breaking change, and the one change it lets through besides descriptions,
//! an added optional property.

use super::breaking::breaking_changes;
use serde_json::{Value, json};

#[test]
fn a_narrowed_type_a_new_bound_or_a_new_format_is_refused() {
    let committed = json!({
        "type": "object",
        "properties": {"count": {"type": ["integer", "null"], "format": "uint64", "minimum": 0}}
    });
    let fresh = json!({
        "type": "object",
        "properties": {
            "count": {"type": "string", "format": "uint32", "minimum": 1, "maximum": 9}
        }
    });
    assert_eq!(
        breaking_changes(&committed, &fresh),
        [
            "/count: no longer accepts integer",
            "/count: no longer accepts null",
            "/count: now accepts string",
            "/count: minimum raised from 0 to 1",
            "/count: maximum 9 added",
            "/count: format \"uint32\" where it was \"uint64\"",
        ]
    );
    let untyped = json!({"description": "Any value."});
    assert_eq!(
        breaking_changes(&untyped, &json!({"type": "string"})),
        ["accepts only string now"]
    );
}

#[test]
fn a_widened_type_value_format_or_bound_is_refused() {
    let committed = json!({
        "type": "object",
        "properties": {
            "count": {"type": "integer", "format": "uint64", "minimum": 5, "maximum": 10},
            "state": {"type": "string", "enum": ["a"]}
        },
        "required": ["count"]
    });
    let fresh = json!({
        "type": "object",
        "properties": {
            "count": {"type": ["integer", "null"], "minimum": 0},
            "state": {"type": "string", "enum": ["a", "b"]}
        },
        "required": ["count"]
    });
    assert_eq!(
        breaking_changes(&committed, &fresh),
        [
            "/count: now accepts null",
            "/count: minimum lowered from 5 to 0",
            "/count: maximum 10 removed",
            "/count: format none where it was \"uint64\"",
            "/state: now accepts \"b\"",
        ]
    );
    let text = json!({"type": "string", "enum": ["a"]});
    assert_eq!(
        breaking_changes(&text, &json!({"type": "string"})),
        ["accepts any value now"]
    );
    assert_eq!(
        breaking_changes(&text, &json!({"enum": ["a"]})),
        Vec::<String>::new(),
        "the values it lists still give its type"
    );
    assert_eq!(
        breaking_changes(&json!({"type": "string"}), &json!({})),
        ["accepts any type now"]
    );
}

#[test]
fn an_enum_value_gone_or_added_is_refused_in_either_form() {
    // schemars writes an enum whose variants are documented as a oneOf of
    // constants, and one whose variants are not as an enum.
    let documented = json!({"oneOf": [
        {"type": "string", "const": "a", "description": "A."},
        {"type": "string", "const": "b", "description": "B."}
    ]});
    let listed = json!({"type": "string", "enum": ["a", "c"]});
    assert_eq!(
        breaking_changes(&documented, &listed),
        ["no longer accepts \"b\"", "now accepts \"c\""]
    );
    assert_eq!(
        breaking_changes(&listed, &documented),
        ["no longer accepts \"c\"", "now accepts \"b\""]
    );
    let same = json!({"type": "string", "enum": ["a", "b"]});
    assert_eq!(breaking_changes(&documented, &same), Vec::<String>::new());
    assert_eq!(
        breaking_changes(&json!({"type": "string"}), &listed),
        ["accepts only \"a\" or \"c\" now"]
    );
}

#[test]
fn a_required_property_made_optional_or_gone_is_refused() {
    let committed = json!({
        "properties": {"a": {"type": "string"}, "b": {"type": "string"}},
        "required": ["a", "b"]
    });
    let fresh = json!({"properties": {"a": {"type": "string"}}});
    assert_eq!(
        breaking_changes(&committed, &fresh),
        ["property b is gone", "property a is no longer required"]
    );
}

#[test]
fn changed_items_or_additional_properties_are_refused() {
    let list = |items: Value| json!({"type": "array", "items": items});
    let texts = list(json!({"type": "string"}));
    assert_eq!(
        breaking_changes(&json!({"type": "array"}), &texts),
        ["accepts fewer items"]
    );
    assert_eq!(
        breaking_changes(&texts, &json!({"type": "array"})),
        ["accepts any items now"]
    );
    assert_eq!(
        breaking_changes(&texts, &list(json!({"type": "integer"}))),
        [
            "/items: no longer accepts string",
            "/items: now accepts integer"
        ]
    );
    let open = json!({"type": "object"});
    let map = |values: Value| json!({"type": "object", "additionalProperties": values});
    for closed in [json!(false), json!({"type": "string"})] {
        assert_eq!(
            breaking_changes(&open, &map(closed.clone())),
            ["accepts fewer additional properties"]
        );
        assert_eq!(
            breaking_changes(&map(closed), &open),
            ["accepts more additional properties"]
        );
    }
    assert_eq!(
        breaking_changes(
            &map(json!({"type": "string"})),
            &map(json!({"type": "integer"}))
        ),
        [
            "/additionalProperties: no longer accepts string",
            "/additionalProperties: now accepts integer"
        ]
    );
    assert_eq!(
        breaking_changes(&map(json!(false)), &map(json!({"type": "string"}))),
        ["accepts more additional properties"]
    );
}

#[test]
fn a_change_the_check_cannot_read_is_refused() {
    assert_eq!(
        breaking_changes(
            &json!({"type": "string"}),
            &json!({"type": "string", "pattern": "^a"})
        ),
        ["pattern changed, which the check does not read"]
    );
    let optional = json!({"anyOf": [{"type": "string"}, {"type": "null"}]});
    assert_eq!(
        breaking_changes(&optional, &json!({"anyOf": [{"type": "string"}]})),
        ["anyOf changed its branches, which the check cannot compare"]
    );
    let item = |properties: Value| {
        json!({
            "anyOf": [{"$ref": "#/$defs/Item"}, {"type": "null"}],
            "$defs": {"Item": {"type": "object", "properties": properties}}
        })
    };
    assert_eq!(
        breaking_changes(&item(json!({"id": {"type": "string"}})), &item(json!({}))),
        ["/anyOf/0: property id is gone"]
    );
    let extra = |minimum: u64| {
        json!({
            "allOf": [{"$ref": "#/$defs/Extra"}],
            "$defs": {"Extra": {"minimum": minimum}}
        })
    };
    assert_eq!(
        breaking_changes(&extra(1), &extra(2)),
        ["definition Extra changed where the check does not follow it"]
    );
    let dangling = json!({"$ref": "#/$defs/Missing"});
    assert_eq!(
        breaking_changes(&dangling, &dangling),
        ["\"#/$defs/Missing\" cannot be followed"]
    );
}

#[test]
fn a_schema_that_only_adds_optional_properties_or_describes_passes() {
    let committed = json!({
        "title": "A",
        "description": "Before.",
        "type": "object",
        "properties": {
            "count": {"type": "integer", "format": "uint64", "minimum": 0},
            "state": {"type": "string", "enum": ["a"]}
        },
        "required": ["count"]
    });
    let fresh = json!({
        "title": "B",
        "description": "After.",
        "type": "object",
        "properties": {
            "count": {"type": "integer", "format": "uint64", "minimum": 0, "description": "New."},
            "state": {"type": "string", "enum": ["a"]},
            "extra": {"type": ["string", "null"]}
        },
        "required": ["count"]
    });
    assert_eq!(breaking_changes(&committed, &fresh), Vec::<String>::new());
}
