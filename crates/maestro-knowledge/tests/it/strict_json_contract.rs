//! Strict acquisition JSON preserves values and enforces defensive boundaries.
use maestro_knowledge::strict_json::{self, MAX_BYTES};
use serde::Deserialize;
use serde_json::{Value, json};
use std::iter::repeat_n;

/// Parse through the public strict resource entry point.
fn parse(text: &str) -> serde_json::Result<Value> {
    strict_json::parse(text.as_bytes())
}

#[test]
fn every_json_kind_is_preserved() {
    let value = json!({"values": [null, true, 18_446_744_073_709_551_615_u64,
        -7, 1.25, "text", [], {}]});
    assert_eq!(parse(&value.to_string()).unwrap(), value);
    assert_eq!(Value::default(), Value::Null);
}

#[test]
fn byte_limit_accepts_boundary_and_refuses_larger_documents() {
    assert_eq!(MAX_BYTES, 4_194_304);
    let mut text = "{}".to_owned();
    text.extend(repeat_n(' ', MAX_BYTES - text.len()));
    assert_eq!(parse(&text).unwrap(), json!({}));
    text.push_str("  ");
    assert!(parse(&text).unwrap_err().to_string().contains("byte limit"));
}

#[test]
fn object_keys_enforce_length_nul_and_duplicate_rules() {
    let boundary = json!({"a".repeat(4096): 1});
    assert_eq!(parse(&boundary.to_string()).unwrap(), boundary);
    for text in [
        json!({"a".repeat(4098): 1}).to_string(),
        json!({"a\0b": 1}).to_string(),
        "{\"a\":1,\"a\":2}".to_owned(),
    ] {
        assert!(
            parse(&text)
                .unwrap_err()
                .to_string()
                .contains("invalid or duplicate field")
        );
    }
}

#[test]
fn string_limit_accepts_boundary_and_refuses_above_it() {
    let value = json!({"text": "a".repeat(8192)});
    assert_eq!(parse(&value.to_string()).unwrap(), value);
    for text in ["a".repeat(8194), "a\0b".to_owned()] {
        assert!(
            parse(&json!({"text": text}).to_string())
                .unwrap_err()
                .to_string()
                .contains("string limit")
        );
    }
}

#[test]
fn map_and_array_nesting_limits_are_independent() {
    for arrays in [false, true] {
        let wrap = |value: Value| {
            if arrays {
                json!([value])
            } else {
                json!({"child": value})
            }
        };
        let mut value = json!(0);
        for _ in 0..31 {
            value = wrap(value);
        }
        let boundary = json!({"root": value});
        assert_eq!(parse(&boundary.to_string()).unwrap(), boundary);
        let excessive = json!({"root": wrap(boundary["root"].clone())});
        assert!(
            parse(&excessive.to_string())
                .unwrap_err()
                .to_string()
                .contains("nesting limit")
        );
    }
}

#[test]
fn cumulative_items_are_charged_across_containers() {
    let boundary = json!({"first": vec![0; 9999], "second": vec![0; 9999]});
    assert_eq!(parse(&boundary.to_string()).unwrap(), boundary);
    let excessive = json!({"first": vec![0; 10000], "second": vec![0; 10000]});
    assert!(
        parse(&excessive.to_string())
            .unwrap_err()
            .to_string()
            .contains("item limit")
    );
}

/// A record exercising both required nullable field decoders.
#[derive(Debug, Deserialize, PartialEq)]
struct NullableFields {
    /// Present object or explicit null.
    #[serde(deserialize_with = "strict_json::nullable_object")]
    object: Option<Value>,
    /// Present scalar or explicit null.
    #[serde(deserialize_with = "strict_json::nullable")]
    scalar: Option<i64>,
}

#[test]
fn nullable_fields_preserve_present_values() {
    let parsed: NullableFields =
        strict_json::parse(br#"{"object":{"key":true},"scalar":-7}"#).unwrap();
    assert_eq!(
        parsed,
        NullableFields {
            object: Some(json!({"key":true})),
            scalar: Some(-7)
        }
    );
    let nulls: NullableFields = strict_json::parse(br#"{"object":null,"scalar":null}"#).unwrap();
    assert_eq!(
        nulls,
        NullableFields {
            object: None,
            scalar: None
        }
    );
}
