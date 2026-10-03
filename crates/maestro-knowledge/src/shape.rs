//! The JSON shapes the contracts name, and no other: an object where a
//! contract names an object, a string where it names one of its values, a
//! scope name where it names an id, and a SHA-256 digest's hexadecimal text
//! where it names a digest. Serde's derive would also read a
//! struct from an array of its fields in order, and `serde_json` a unit
//! variant from an object such as `{"public": null}`; the declaration and
//! the corpus manifest read their objects, their named values and their ids
//! through these instead.

pub(crate) use crate::strict_json::{name, object, objects};
use maestro_kernel::{artifact::Digest, scope};
use serde::{
    Deserialize, Deserializer,
    de::{self, DeserializeOwned},
};

/// `T` from `text`, which holds one JSON object and nothing after it.
pub(crate) fn parse<T: DeserializeOwned>(text: &str) -> serde_json::Result<T> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = object(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

/// A source id from a JSON string that is a scope name
/// ([`scope::check_name`]), so that it forms a segment of its scope path.
pub(crate) fn id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let id = String::deserialize(deserializer)?;
    scope::check_name(&id).map_err(de::Error::custom)?;
    Ok(id)
}

/// A collection id from a JSON string that follows the collection-name rule
/// ([`scope::check_collection_name`]).
pub(crate) fn collection_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let id = String::deserialize(deserializer)?;
    scope::check_collection_name(&id).map_err(de::Error::custom)?;
    Ok(id)
}

/// A SHA-256 digest, as the kernel's [`Digest`], from a JSON string of the
/// 64 lowercase hexadecimal characters that alone make one.
pub(crate) fn digest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Digest, D::Error> {
    let text = String::deserialize(deserializer)?;
    Digest::parse(&text).map_err(de::Error::custom)
}
