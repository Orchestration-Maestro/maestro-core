//! The JSON shapes the contracts name, and no other: an object where a
//! contract names an object, and a string where it names one of its values.
//! Serde's derive would also read a struct from an array of its fields in
//! order, and `serde_json` a unit variant from an object such as
//! `{"public": null}`; the declaration and the corpus manifest read their
//! objects and their named values through these instead.

use serde::{
    Deserialize, Deserializer,
    de::{
        DeserializeOwned, MapAccess, Visitor,
        value::{MapAccessDeserializer, StringDeserializer},
    },
};
use std::{fmt, marker::PhantomData};

/// `T` from `text`, which holds one JSON object and nothing after it.
pub(crate) fn parse<T: DeserializeOwned>(text: &str) -> serde_json::Result<T> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = object(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

/// `T` from a JSON object only.
pub(crate) fn object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_map(FromObject(PhantomData))
}

/// A list of `T`, each from a JSON object only.
pub(crate) fn objects<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let items = Vec::<Object<T>>::deserialize(deserializer)?;
    Ok(items.into_iter().map(|Object(item)| item).collect())
}

/// `T`, one of the values a contract names, from a JSON string only.
pub(crate) fn name<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let text = String::deserialize(deserializer)?;
    T::deserialize(StringDeserializer::new(text))
}

/// A list element read by [`object`].
struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        object(deserializer).map(Self)
    }
}

/// Reads `T` from the entries of a JSON object.
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
