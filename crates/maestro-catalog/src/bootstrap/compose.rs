//! Resolve explicit preset names through a replaceable source port.
use crate::source::json;
use crate::{files::FileInput, limits::Limits, source::bootstrap_inventory::validate_source_path};
use maestro_filesystem::Directory;
use std::{
    collections::{BTreeMap, BTreeSet},
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
    /// The checked, immutable manifest-backed lowest defaults slot.
    ///
    /// # Errors
    /// Refuses invalid defaults before composing outputs.
    fn defaults(&self) -> Result<maestro_settings::Registry, String> {
        maestro_settings::Registry::built_in().map_err(|error| error.to_string())
    }

    /// Missing runtime inputs are pinned too, so adding one invalidates preview.
    fn absent_inputs(&self) -> Vec<(PathBuf, String)> {
        Vec::new()
    }

    /// Runtime base types captured alongside frozen defaults.
    fn backend_types(&self) -> BTreeSet<String> {
        BTreeSet::new()
    }

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
    json::parse(bytes)
        .map(|_| ())
        .map_err(|error| format!("generated JSON {path} is invalid: {error}"))
}
