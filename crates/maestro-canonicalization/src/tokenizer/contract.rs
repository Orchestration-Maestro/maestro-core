//! The committed qualification profile: its identity and typed access to its fields.
use crate::error::Error;
use crate::hashing::digest;
use serde_json::Value;
use std::collections::BTreeMap;

/// The committed profile's identifier: the SHA-256 of its canonical JSON without this field.
pub(super) const CONTRACT_ID: &str =
    "sha256:3546447555757daa4996a2e2e708bc67bce4389e8cee3f8386ed503eeaa6d01c";

/// The refusal for a profile that lacks a field or has the wrong type.
pub(super) fn invalid_contract() -> Error {
    Error("invalid or unqualified tokenizer contract".into())
}

/// The string at a JSON pointer in the profile.
pub(super) fn text_at<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, Error> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(invalid_contract)
}

/// The array at a JSON pointer in the profile.
pub(super) fn array_at<'a>(value: &'a Value, pointer: &str) -> Result<&'a [Value], Error> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(invalid_contract)
}

/// The JSON value with object keys sorted at every level, for one canonical serialization.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let ordered: BTreeMap<_, _> = map
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// Parse the profile: its declared identifier must equal the compiled one and the digest of its
/// sorted JSON without that field.
pub(super) fn parse_contract(text: &str) -> Result<Value, Error> {
    let mut value: Value = serde_json::from_str(text).map_err(|_| invalid_contract())?;
    let declared = value
        .as_object_mut()
        .and_then(|map| map.remove("contract_id"))
        .ok_or_else(invalid_contract)?;
    if declared.as_str() != Some(CONTRACT_ID) {
        return Err(invalid_contract());
    }
    let mut value = sorted(value);
    let bytes = serde_json::to_vec(&value).map_err(|_| invalid_contract())?;
    if format!("sha256:{}", digest(&bytes)) != CONTRACT_ID {
        return Err(invalid_contract());
    }
    value
        .as_object_mut()
        .ok_or_else(invalid_contract)?
        .insert("contract_id".into(), declared);
    Ok(value)
}

/// The value at a JSON pointer in the profile.
pub(super) fn value_at<'a>(value: &'a Value, pointer: &str) -> Result<&'a Value, Error> {
    value.pointer(pointer).ok_or_else(invalid_contract)
}
