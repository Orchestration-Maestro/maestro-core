//! A corpus manifest: `maestro-corpus/1`, one JSON line per document, through
//! which S1 imports an existing corpus (docs/architecture/01 §2.1, plan D15).
//!
//! A line is strict as a declaration is: every key is one the contract names
//! and appears once in its object, at every depth of `extractor` and `access`
//! too, whose contents the contract leaves open, and the line is one JSON value
//! with no number out of range. `path` stays inside the manifest's directory,
//! `sha256` is 64 lowercase hexadecimal characters, `bytes` is a positive
//! whole number, and `source_ref`, the document's identity, is its origin URL
//! or, for a document without one, `corpus-path:` followed by the line's own
//! `path`. Whether the file holds those bytes is the importer's check, when it
//! reads the file.

use crate::relative_path::RelativePath;
use maestro_kernel::artifact::Digest;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};
use std::{error, fmt, num::NonZeroU64, str::FromStr};

/// The `source_ref` prefix of a document without a URL.
const CORPUS_PATH: &str = "corpus-path:";

/// One document of a corpus manifest. [`str::parse`] reads a line and checks
/// its `source_ref` against its `path`, which the shape alone cannot.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Entry {
    /// The contract the line follows.
    pub schema: Schema,
    /// The document's file, relative to the manifest's directory.
    pub path: RelativePath,
    /// The SHA-256 digest of the file's bytes.
    #[serde(deserialize_with = "digest")]
    pub sha256: Digest,
    /// The file's size in bytes.
    pub bytes: NonZeroU64,
    /// The document's identity: its origin URL, `http` or `https`, or
    /// `corpus-path:` followed by [`Entry::path`] when it has none.
    pub source_ref: String,
    /// The document's title.
    pub title: String,
    /// The kind of source the document comes from, such as `docs`.
    pub source_kind: String,
    /// The document set it belongs to.
    pub set: Option<String>,
    /// The release it documents.
    pub version: Option<String>,
    /// Its language.
    pub lang: Option<String>,
    /// When it was captured.
    pub captured_at: Option<String>,
    /// The product it documents.
    pub product: Option<String>,
    /// The component it documents.
    pub component: Option<String>,
    /// The platform it applies to.
    pub platform: Option<String>,
    /// How it was extracted, kept as given.
    #[serde(default, deserialize_with = "distinct_object")]
    pub extractor: Option<Map<String, Value>>,
    /// Who may read it and under which licence, kept as given.
    #[serde(default, deserialize_with = "distinct_object")]
    pub access: Option<Map<String, Value>>,
}

impl Entry {
    /// Whether `source_ref` is a web URL, or `corpus-path:` followed by the
    /// line's own path.
    fn source_ref_holds(&self) -> bool {
        match self.source_ref.strip_prefix(CORPUS_PATH) {
            Some(path) => path == self.path.as_str(),
            None => is_web_url(&self.source_ref),
        }
    }
}

impl FromStr for Entry {
    type Err = Error;

    /// The document one manifest line declares.
    ///
    /// # Errors
    ///
    /// [`Error::Json`] when the line is not strict JSON of the contract's
    /// shape, and [`Error::SourceRef`] when its `source_ref` is neither a web
    /// URL nor its own corpus path.
    fn from_str(line: &str) -> Result<Self, Error> {
        let entry: Self = serde_json::from_str(line).map_err(Error::Json)?;
        if entry.source_ref_holds() {
            Ok(entry)
        } else {
            Err(Error::SourceRef(entry.source_ref))
        }
    }
}

/// The contract a line follows; this version reads the first only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Schema {
    /// `maestro-corpus/1`.
    #[serde(rename = "maestro-corpus/1")]
    V1,
}

/// Why a line is not a `maestro-corpus/1` entry.
#[derive(Debug)]
pub enum Error {
    /// Not strict JSON of the contract's shape: not one JSON value, a number
    /// out of range, an unknown, repeated or missing key, a value the contract
    /// does not allow or a path that leaves the manifest's directory.
    Json(serde_json::Error),
    /// This `source_ref` is neither a web URL nor `corpus-path:` followed by
    /// the line's own path.
    SourceRef(String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "not a strict maestro-corpus/1 line: {error}"),
            Self::SourceRef(source_ref) => write!(
                formatter,
                "the source_ref {source_ref:?} is neither an http or https URL with a host \
                 nor {CORPUS_PATH} followed by the line's own path"
            ),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::SourceRef(_) => None,
        }
    }
}

/// Whether `text` is an `http` or `https` URL with a host, holding no
/// whitespace or control character.
fn is_web_url(text: &str) -> bool {
    let Some(rest) = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
    else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !host.is_empty()
        && !text
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

/// `sha256` as the kernel's [`Digest`], which only 64 lowercase hexadecimal
/// characters make.
fn digest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Digest, D::Error> {
    let text = String::deserialize(deserializer)?;
    Digest::parse(&text).map_err(de::Error::custom)
}

/// `extractor` or `access`: an object the contract leaves open, read with
/// every key distinct at every depth, where a map alone would keep the last
/// of two; absent or `null`, none.
fn distinct_object<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Map<String, Value>>, D::Error> {
    Option::<DistinctObject>::deserialize(deserializer)
        .map(|object| object.map(|DistinctObject(entries)| entries))
}

/// A JSON object read by [`ObjectVisitor`].
struct DistinctObject(Map<String, Value>);

impl<'de> Deserialize<'de> for DistinctObject {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ObjectVisitor).map(Self)
    }
}

/// A JSON value read by [`ValueVisitor`].
struct DistinctValue(Value);

impl<'de> Deserialize<'de> for DistinctValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor).map(Self)
    }
}

/// Reads a JSON object whose keys are distinct at every depth.
struct ObjectVisitor;

impl<'de> Visitor<'de> for ObjectVisitor {
    type Value = Map<String, Value>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        distinct_entries(access)
    }
}

/// Reads any JSON value, each object in it with distinct keys.
struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("number out of range"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(DistinctValue(item)) = access.next_element()? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, access: A) -> Result<Value, A::Error> {
        distinct_entries(access).map(Value::Object)
    }
}

/// An object's entries, refused when it holds a key twice.
fn distinct_entries<'de, A: MapAccess<'de>>(mut access: A) -> Result<Map<String, Value>, A::Error> {
    let mut entries = Map::new();
    while let Some(key) = access.next_key::<String>()? {
        if entries.contains_key(&key) {
            return Err(de::Error::custom(format_args!("duplicate key `{key}`")));
        }
        let DistinctValue(value) = access.next_value()?;
        entries.insert(key, value);
    }
    Ok(entries)
}
