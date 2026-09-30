//! Bound and de-duplicate JSON before any typed resource is materialized.
use serde::{
    Deserialize, Deserializer,
    de::{
        self, DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor,
        value::{MapAccessDeserializer, StringDeserializer},
    },
};
use serde_json::{Map, Number, Value};
use std::{
    collections::BTreeSet,
    fmt,
    io::{self, ErrorKind},
    marker::PhantomData,
};

/// Defensive configuration-file size, not an acquisition budget.
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
/// Maximum container nesting in a configuration.
const MAX_DEPTH: usize = 32;
/// Maximum cumulative object members and array elements.
const MAX_ITEMS: usize = 20_000;
/// Maximum elements in one array.
const MAX_ARRAY: usize = 10_000;

/// Parse strictly before typed deserialization can discard duplicate keys.
pub(crate) fn bounded(bytes: &[u8]) -> serde_json::Result<Value> {
    if bytes.len() > MAX_BYTES {
        return Err(serde_json::Error::io(io::Error::new(
            ErrorKind::InvalidData,
            "configuration byte limit",
        )));
    }
    let mut remaining = MAX_ITEMS;
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = Node {
        depth: 0,
        remaining: &mut remaining,
    }
    .deserialize(&mut decoder)?;
    decoder.end()?;
    Ok(value)
}

/// One recursively bounded value, borrowing the document-wide accounting.
struct Node<'a> {
    /// Containers enclosing this value.
    depth: usize,
    /// Remaining members/elements across the whole document.
    remaining: &'a mut usize,
}

impl Node<'_> {
    /// Charge a member before decoding its value.
    fn charge<E: de::Error>(&mut self) -> Result<(), E> {
        *self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| E::custom("configuration item limit"))?;
        Ok(())
    }
    /// Check a container before reading its children.
    fn container<E: de::Error>(&self) -> Result<(), E> {
        if self.depth >= MAX_DEPTH {
            Err(E::custom("configuration nesting limit"))
        } else {
            Ok(())
        }
    }
}

impl<'de> DeserializeSeed<'de> for Node<'_> {
    type Value = Value;
    fn deserialize<D: Deserializer<'de>>(self, decoder: D) -> Result<Value, D::Error> {
        decoder.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Node<'_> {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded strict JSON")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("nonfinite number"))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.into()))
    }
    fn visit_map<A: MapAccess<'de>>(mut self, mut map: A) -> Result<Value, A::Error> {
        self.container()?;
        let mut seen = BTreeSet::new();
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            self.charge()?;
            if key.len() > 4096 || key.contains('\0') || !seen.insert(key.clone()) {
                return Err(de::Error::custom("invalid or duplicate field"));
            }
            let value = map.next_value_seed(Node {
                depth: self.depth + 1,
                remaining: self.remaining,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
    fn visit_seq<A: SeqAccess<'de>>(mut self, mut array: A) -> Result<Value, A::Error> {
        self.container()?;
        let mut values = Vec::new();
        while let Some(value) = array.next_element_seed(Node {
            depth: self.depth + 1,
            remaining: self.remaining,
        })? {
            self.charge()?;
            if values.len() >= MAX_ARRAY {
                return Err(de::Error::custom("configuration array limit"));
            }
            values.push(value);
        }
        Ok(Value::Array(values))
    }
}

/// Deserialize one bounded strict object without discarding duplicate keys.
///
/// # Errors
/// Invalid shapes, duplicate keys and exceeded defensive format limits refuse.
pub fn parse<T: DeserializeOwned>(bytes: &[u8]) -> serde_json::Result<T> {
    let value = bounded(bytes)?;
    strings(&value)?;
    object(value)
}

/// Additional string restrictions for acquisition resources and v2 collections.
pub(crate) fn strings(value: &Value) -> serde_json::Result<()> {
    match value {
        Value::String(text) if text.len() > 8192 || text.contains('\0') => {
            Err(serde_json::Error::io(io::Error::new(
                ErrorKind::InvalidData,
                "configuration string limit",
            )))
        }
        Value::Object(fields) => fields.values().try_for_each(strings),
        Value::Array(items) => items.iter().try_for_each(strings),
        _ => Ok(()),
    }
}
/// Decode only a JSON object into a typed record.
///
/// # Errors
/// Wrong JSON shapes or invalid typed values refuse.
pub fn object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(decoder: D) -> Result<T, D::Error> {
    decoder.deserialize_map(FromObject(PhantomData))
}
/// Decode a sequence of objects, never arrays posing as records.
///
/// # Errors
/// Wrong JSON shapes or invalid typed values refuse.
pub fn objects<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    decoder: D,
) -> Result<Vec<T>, D::Error> {
    Vec::<Object<T>>::deserialize(decoder)
        .map(|items| items.into_iter().map(|Object(item)| item).collect())
}
/// A nullable field is still required; a present non-null value is an object.
///
/// # Errors
/// Wrong JSON shapes or invalid typed values refuse.
pub fn nullable_object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    decoder: D,
) -> Result<Option<T>, D::Error> {
    Option::<Object<T>>::deserialize(decoder).map(|value| value.map(|Object(item)| item))
}
/// Required nullable scalar, not an implicit missing default.
///
/// # Errors
/// Wrong JSON shapes or invalid typed values refuse.
pub fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    decoder: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(decoder)
}
/// Named enum values must be strings, not unit-variant objects.
///
/// # Errors
/// Wrong JSON shapes or invalid typed values refuse.
pub fn name<'de, D: Deserializer<'de>, T: Deserialize<'de>>(decoder: D) -> Result<T, D::Error> {
    T::deserialize(StringDeserializer::new(String::deserialize(decoder)?))
}
/// A list of string-valued enum variants.
///
/// # Errors
/// Wrong JSON shapes or invalid typed values refuse.
pub fn names<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    decoder: D,
) -> Result<Vec<T>, D::Error> {
    Vec::<String>::deserialize(decoder)?
        .into_iter()
        .map(|text| T::deserialize(StringDeserializer::new(text)))
        .collect()
}
/// Force serde struct values through object shape.
struct Object<T>(T);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        object(decoder).map(Self)
    }
}
/// A map-only record decoder.
struct FromObject<T>(PhantomData<T>);
impl<'de, T: Deserialize<'de>> Visitor<'de> for FromObject<T> {
    type Value = T;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }
    fn visit_map<A: MapAccess<'de>>(self, entries: A) -> Result<T, A::Error> {
        T::deserialize(MapAccessDeserializer::new(entries))
    }
}
