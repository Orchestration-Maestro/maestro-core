//! Explicit sorted-object JSON for stable identities and presentation.

use serde::{Serialize, Serializer, ser::Error as _};
use serde_json::Value;

/// Sorts object keys recursively, preserving array order and scalar values.
/// Any new digest over JSON must use this boundary, never transitive map features.
#[must_use]
pub fn canonical(mut value: Value) -> Value {
    value.sort_all_objects();
    value
}

/// Serializes only an opaque JSON field with sorted object keys.
/// Typed enclosing records retain their declared field order.
///
/// # Errors
/// Returns conversion or serializer errors without dropping the field.
pub fn serialize_canonical<T: Serialize, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let value = serde_json::to_value(value).map_err(S::Error::custom)?;
    canonical(value).serialize(serializer)
}

#[cfg(test)]
mod tests {
    use super::canonical;
    use serde_json::{Map, Value, json};

    #[test]
    fn canonical_bytes_sort_reverse_ordered_maps_recursively_not_arrays() {
        let mut object = Map::new();
        object.insert("z".into(), json!([{"z": 2, "a": 1}, 3, 1]));
        object.insert("a".into(), json!(0));
        assert_eq!(
            canonical(Value::Object(object)).to_string().as_bytes(),
            br#"{"a":0,"z":[{"a":1,"z":2},3,1]}"#,
        );
    }
}
