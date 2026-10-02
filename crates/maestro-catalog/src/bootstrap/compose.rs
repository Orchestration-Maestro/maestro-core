//! Resolve explicit preset names through a replaceable source port.
use crate::{files::FileInput, limits::Limits, source::bootstrap_inventory::validate_source_path};
use maestro_filesystem::Directory;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Path, PathBuf},
};

/// Relative file names mapped to exact inert bytes.
pub(super) type FileBytes = BTreeMap<String, Vec<u8>>;

/// One checked source's exact bytes and resource provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    /// Qualified identity of the resource owning this source, sidecar or asset.
    pub id: String,
    /// Declared resource revision, absent only for unversioned sources.
    pub revision: Option<String>,
    /// Bytes from the single checked snapshot.
    pub bytes: Vec<u8>,
}

/// One named preset's root-relative file set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preset {
    /// The declared preset identifier.
    pub name: String,
    /// Files selected by this preset.
    pub files: FileBytes,
    /// Complete checked source closure, keyed by catalog-relative path.
    pub source_files: BTreeMap<String, SourceFile>,
    /// Exact selected area identities, sorted and included once across presets.
    pub areas: Vec<String>,
    /// Executable names to report without invoking them.
    pub tools: Vec<String>,
    /// Required binding references, reported without resolving or invoking them.
    pub bindings: Vec<String>,
    /// Captured catalog root for input revalidation; absent for non-filesystem ports.
    pub source_root: Option<PathBuf>,
}

/// Replaceable source of preset definitions and their inert file bytes.
pub trait PresetPort {
    /// Resolve the selected preset closure in stable ID order; unknown names are errors.
    ///
    /// # Errors
    /// Returns an error for missing or invalid preset definitions.
    fn resolve(&self, names: &[String]) -> Result<Vec<Preset>, String>;
}

/// Read every manifest and template through held handles and refuse oversized bytes.
pub(super) fn read_preset_file(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    validate_source_path(relative)?;
    let path = Path::new(relative);
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid preset path: {relative}"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid preset path: {relative}"))?;
    Directory::open(root, parent, false)
        .and_then(|directory| {
            directory.read_regular_bounded(name, Limits::PRODUCTION.source_file_bytes)
        })
        .map_err(|error| format!("cannot read {relative}: {error}"))
}

/// Compose already-resolved presets and return their generated files.
pub(super) fn compose_resolved(presets: Vec<Preset>) -> Result<Vec<FileInput>, String> {
    let mut selected = BTreeSet::new();
    let mut files = BTreeMap::new();
    for preset in presets {
        if !selected.insert(preset.name.clone()) {
            return Err(format!("preset selected more than once: {}", preset.name));
        }
        for (path, bytes) in preset.files {
            if files.insert(path.clone(), bytes).is_some() {
                return Err(format!("preset file collision: {path}"));
            }
        }
    }
    for (path, bytes) in &files {
        if Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            validate_json(path, bytes)?;
        }
    }
    Ok(files
        .into_iter()
        .map(|(path, bytes)| FileInput::new(path, bytes))
        .collect())
}

/// Reject malformed or duplicate-key generated JSON before publication.
pub(super) fn validate_json(path: &str, bytes: &[u8]) -> Result<(), String> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    StrictJson::deserialize(&mut deserializer)
        .map(|_| ())
        .and_then(|()| deserializer.end())
        .map_err(|error| format!("generated JSON {path} is invalid: {error}"))
}

/// A recursively checked JSON value that refuses duplicate object keys.
struct StrictJson;

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
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self::Value, E> {
        Ok(StrictJson)
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self::Value, E> {
        Ok(StrictJson)
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self::Value, E> {
        Ok(StrictJson)
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self::Value, E> {
        Ok(StrictJson)
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<Self::Value, E> {
        Ok(StrictJson)
    }
    fn visit_string<E: de::Error>(self, _: String) -> Result<Self::Value, E> {
        Ok(StrictJson)
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(StrictJson)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        while sequence.next_element::<StrictJson>()?.is_some() {}
        Ok(StrictJson)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom(format!("duplicate object key {key:?}")));
            }
            map.next_value::<StrictJson>()?;
        }
        Ok(StrictJson)
    }
}
