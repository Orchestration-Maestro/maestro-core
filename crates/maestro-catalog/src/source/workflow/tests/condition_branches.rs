//! Previously unexecuted condition branches, each with an exact typed neighbour.

use super::contracts::check;
use serde_json::json;

#[test]
fn condition_trailing_whitespace_and_unicode_escapes_keep_string_types() {
    let schema = json!({"type": "object"});
    assert_eq!(check("true \t\n", &schema), Ok(()));
    assert!(
        check("true \t\n false", &schema)
            .unwrap_err()
            .contains("complete boolean")
    );
    for text in [r#"'\u0041' == "A""#, r#"'\n' == "\t""#, r#"'\'' == "\"""#] {
        assert_eq!(check(text, &schema), Ok(()), "{text}");
    }
    assert!(
        check(r#"'\u004z' == "A""#, &schema)
            .unwrap_err()
            .contains("invalid unicode escape")
    );
}

#[test]
fn condition_field_access_requires_object_and_accepts_null_scalars() {
    let mut schema = json!({"type": "object", "required": ["result"],
        "properties": {"result": {"type": "null"}}});
    assert_eq!(check("result == null", &schema), Ok(()));
    assert!(
        check("result == false", &schema)
            .unwrap_err()
            .contains("matching scalar types")
    );
    assert!(
        check("result.value == null", &schema)
            .unwrap_err()
            .contains("value needs an object")
    );
    schema["properties"]["result"] = json!({"type": "object", "required": ["value"],
        "properties": {"value": {"type": "null"}}});
    assert_eq!(check("result.value == null", &schema), Ok(()));
}

#[test]
fn condition_ambiguous_field_types_refuse_instead_of_coercing() {
    let mut schema = json!({"type": "object", "required": ["result"],
        "properties": {"result": {"type": "boolean"}}});
    assert_eq!(check("result", &schema), Ok(()));
    for field in [
        json!({}),
        json!({"type": ["boolean", "null"]}),
        json!({"type": "invented"}),
    ] {
        schema["properties"]["result"] = field;
        assert!(
            check("result", &schema)
                .unwrap_err()
                .contains("unambiguous contract type")
        );
    }
}
