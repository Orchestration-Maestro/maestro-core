//! Resolve explicit preset names through a replaceable source port.
use crate::{files::FileInput, limits::Limits, source::tree::read_bounded};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::Path,
    str,
};

/// Relative file names mapped to exact inert bytes.
type FileBytes = BTreeMap<String, Vec<u8>>;

/// One named preset's root-relative file set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preset {
    /// The declared preset identifier.
    pub name: String,
    /// Files selected by this preset.
    pub files: FileBytes,
    /// Source manifest and template files, keyed by catalog-relative path.
    pub source_files: FileBytes,
}

/// Replaceable source of preset definitions and their inert file bytes.
pub trait PresetPort {
    /// Resolve all requested presets in order; unknown names are errors.
    ///
    /// # Errors
    /// Returns an error for missing or invalid preset definitions.
    fn resolve(&self, names: &[String]) -> Result<Vec<Preset>, String>;
}

/// Filesystem adapter for explicit preset manifests beneath `bootstrap/`.
#[derive(Debug)]
pub struct DirectoryPresets<'a> {
    /// Catalog directory to read without executing its content.
    root: &'a Path,
}

impl<'a> DirectoryPresets<'a> {
    /// Read preset definitions and their inert files from a catalog directory.
    #[must_use]
    pub const fn new(root: &'a Path) -> Self {
        Self { root }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
/// Strict manifest naming the inert source files in one preset.
struct PresetManifest {
    /// The preset identifier, which must match the requested name.
    name: String,
    /// Source files beneath the `base/` or `rust/` overlay directories.
    files: Vec<String>,
}

impl PresetPort for DirectoryPresets<'_> {
    fn resolve(&self, names: &[String]) -> Result<Vec<Preset>, String> {
        names
            .iter()
            .map(|name| load_preset(self.root, name))
            .collect()
    }
}

/// Read and validate one requested preset.
fn load_preset(root: &Path, name: &str) -> Result<Preset, String> {
    let manifest_path = format!("bootstrap/{name}.toml");
    let bytes = read_bounded(root, &manifest_path, Limits::PRODUCTION.source_file_bytes)
        .map_err(|error| error.to_string())?;
    let text = str::from_utf8(&bytes).map_err(|error| error.to_string())?;
    let manifest: PresetManifest = toml::from_str(text).map_err(|error| error.to_string())?;
    if manifest.name != name {
        return Err(format!("preset name mismatch in {manifest_path}"));
    }
    let (files, mut source_files) = load_preset_files(root, manifest.files)?;
    source_files.insert(manifest_path, bytes);
    Ok(Preset {
        name: name.to_owned(),
        files,
        source_files,
    })
}

/// Read a preset's files, mapping both overlays to project-root-relative paths.
fn load_preset_files(root: &Path, sources: Vec<String>) -> Result<(FileBytes, FileBytes), String> {
    let mut source_files = BTreeMap::new();
    let files = sources
        .into_iter()
        .map(|source| {
            let relative = source
                .strip_prefix("base/")
                .or_else(|| source.strip_prefix("rust/"))
                .ok_or_else(|| format!("preset file must be beneath base/ or rust/: {source}"))?;
            validate_source_path(relative)?;
            let source_path = format!("bootstrap/{source}");
            let bytes = read_bounded(root, &source_path, Limits::PRODUCTION.source_file_bytes)
                .map_err(|error| error.to_string())?;
            source_files.insert(source_path, bytes.clone());
            Ok((relative.to_owned(), bytes))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    Ok((files, source_files))
}

/// Refuse path components that could leave a preset's overlay directory.
fn validate_source_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains(['\\', ':'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(format!("unsafe preset file path: {path}"));
    }
    Ok(())
}

/// Compose selected presets, rejecting duplicate selection, file collisions and invalid JSON.
///
/// # Errors
/// Returns a diagnostic for missing presets, collisions or invalid generated JSON.
pub fn compose(port: &dyn PresetPort, names: &[String]) -> Result<Vec<FileInput>, String> {
    if names.is_empty() {
        return Err("select at least one preset".to_owned());
    }
    compose_resolved(port.resolve(names)?)
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
