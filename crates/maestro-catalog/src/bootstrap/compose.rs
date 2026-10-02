//! Resolve explicit preset names through a replaceable source port.
use crate::{files::FileInput, limits::Limits};
use maestro_filesystem::Directory;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Path, PathBuf},
    str,
};

/// Relative file names mapped to exact inert bytes.
pub(super) type FileBytes = BTreeMap<String, Vec<u8>>;

/// One named preset's root-relative file set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preset {
    /// The declared preset identifier.
    pub name: String,
    /// Files selected by this preset.
    pub files: FileBytes,
    /// Source manifest and template files, keyed by catalog-relative path.
    pub source_files: FileBytes,
    /// Executable names to report without invoking them.
    pub tools: Vec<String>,
    /// Required binding references, reported without resolving or invoking them.
    pub bindings: Vec<String>,
    /// Captured catalog root for input revalidation; absent for non-filesystem ports.
    pub source_root: Option<PathBuf>,
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
    /// One top-level overlay directory beneath bootstrap.
    overlay: String,
    /// Required executable names, never commands or paths.
    tools: Vec<String>,
    /// Source files beneath the declared overlay directory.
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
    let bytes = read_preset_file(root, &manifest_path)?;
    let text = str::from_utf8(&bytes).map_err(|error| error.to_string())?;
    let manifest: PresetManifest = toml::from_str(text).map_err(|error| error.to_string())?;
    if manifest.name != name {
        return Err(format!("preset name mismatch in {manifest_path}"));
    }
    validate_top_level_name(&manifest.overlay)?;
    for tool in &manifest.tools {
        validate_top_level_name(tool)?;
    }
    let (files, mut source_files) = load_preset_files(root, &manifest.overlay, manifest.files)?;
    source_files.insert(manifest_path, bytes);
    Ok(Preset {
        name: name.to_owned(),
        files,
        source_files,
        tools: manifest.tools,
        bindings: Vec::new(),
        source_root: None,
    })
}

/// Read a preset's files, mapping both overlays to project-root-relative paths.
fn load_preset_files(
    root: &Path,
    overlay: &str,
    sources: Vec<String>,
) -> Result<(FileBytes, FileBytes), String> {
    let mut source_files = BTreeMap::new();
    let files = sources
        .into_iter()
        .map(|source| {
            let relative = source
                .strip_prefix(&format!("{overlay}/"))
                .ok_or_else(|| format!("preset file must be beneath {overlay}/: {source}"))?;
            validate_source_path(relative)?;
            let source_path = format!("bootstrap/{source}");
            let bytes = read_preset_file(root, &source_path)?;
            source_files.insert(source_path, bytes.clone());
            Ok((relative.to_owned(), bytes))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    Ok((files, source_files))
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

/// An overlay or executable is a single name, not a path or command line.
pub(super) fn validate_top_level_name(name: &str) -> Result<(), String> {
    validate_source_path(name)?;
    if name.contains('/') || name.chars().any(char::is_whitespace) {
        return Err(format!("expected a single top-level name: {name}"));
    }
    Ok(())
}

/// Refuse path components that could leave a preset's overlay directory.
pub(super) fn validate_source_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains(['\\', ':'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(format!("unsafe preset file path: {path}"));
    }
    Ok(())
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
