//! The single strict JSON decoder shared by source admission and bootstrap.

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

/// Decode once, retaining the value while refusing duplicate keys and trailing data.
pub(crate) fn parse(bytes: &[u8]) -> Result<Value, String> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = StrictJson::deserialize(&mut deserializer).map_err(|error| error.to_string())?;
    deserializer.end().map_err(|error| error.to_string())?;
    Ok(value.0)
}

/// A recursively checked JSON value that refuses duplicate object keys.
struct StrictJson(Value);

impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictJsonVisitor)
    }
}

/// Validate each JSON value and every object key recursively.
struct StrictJsonVisitor;

impl<'de> Visitor<'de> for StrictJsonVisitor {
    type Value = StrictJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::Bool(value)))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::Number(value.into())))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::Number(value.into())))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Number::from_f64(value)
            .map(|number| StrictJson(Value::Number(number)))
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::String(value.to_owned())))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::String(value)))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::Null))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(value) = sequence.next_element::<StrictJson>()? {
            items.push(value.0);
        }
        Ok(StrictJson(Value::Array(items)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut items = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if items.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate object key {key:?}")));
            }
            items.insert(key, map.next_value::<StrictJson>()?.0);
        }
        Ok(StrictJson(Value::Object(items)))
    }
}

#[cfg(test)]
mod tests {
    use super::{StrictJson, parse};
    use crate::{limits::Limits, policy::schema::bound_json};
    use serde::de::{
        Deserialize,
        value::{BytesDeserializer, Error, F64Deserializer, StringDeserializer},
    };
    use serde_json::json;

    #[test]
    fn strict_json_preserves_scalar_number_and_container_forms() {
        let text =
            br#"[true,false,-1,18446744073709551615,1.25,1e2,"plain","escaped\ntext",null,[],{}]"#;
        assert_eq!(
            parse(text).unwrap(),
            json!([
                true,
                false,
                -1,
                u64::MAX,
                1.25,
                100.0,
                "plain",
                "escaped\ntext",
                null,
                [],
                {}
            ])
        );
        let owned = StringDeserializer::<Error>::new("owned text".to_owned());
        assert_eq!(
            StrictJson::deserialize(owned).unwrap().0,
            json!("owned text")
        );
    }

    #[test]
    fn strict_json_refuses_duplicates_trailing_data_and_invalid_numbers() {
        for text in [r#"{"x":1,"x":2}"#, r#"[{"x":1,"x":2}]"#] {
            assert!(
                parse(text.as_bytes())
                    .unwrap_err()
                    .contains("duplicate object key \"x\"")
            );
        }
        assert!(parse(br#"{"x":1,"y":2} "#).is_ok());
        assert!(
            parse(br#"{"x":1} false"#)
                .unwrap_err()
                .contains("trailing characters")
        );
        for text in ["01", "+1", "1.", "NaN", "Infinity", "1e999"] {
            assert!(parse(text.as_bytes()).is_err(), "accepted {text}");
        }
        for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = StrictJson::deserialize(F64Deserializer::<Error>::new(number))
                .err()
                .unwrap()
                .to_string();
            assert_eq!(error, "non-finite JSON number");
        }
        let error = StrictJson::deserialize(BytesDeserializer::<Error>::new(b"bytes"))
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("expected a JSON value"), "{error}");
    }

    #[test]
    fn strict_json_bounds_have_exact_size_and_depth_neighbours() {
        let limits = Limits {
            source_file_bytes: 5,
            source_depth: 2,
            ..Limits::PRODUCTION
        };
        assert_eq!(bound_json("[[0]]", &limits), Ok(()));
        assert_eq!(parse(b"[[0]]").unwrap(), json!([[0]]));
        assert!(
            bound_json("[[0]] ", &limits)
                .unwrap_err()
                .contains("larger than 5 bytes")
        );
        let limits = Limits {
            source_file_bytes: 7,
            ..limits
        };
        assert!(
            bound_json("[[[0]]]", &limits)
                .unwrap_err()
                .contains("JSON depth exceeds 2 levels")
        );
        let limits = Limits {
            source_depth: 3,
            ..limits
        };
        assert_eq!(bound_json("[[[0]]]", &limits), Ok(()));
        assert_eq!(parse(b"[[[0]]]").unwrap(), json!([[[0]]]));
    }
}
