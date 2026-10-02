//! Explicit area-local inventory data behind the existing preset port.
use super::compose::{
    FileBytes, Preset, PresetPort, read_preset_file, validate_source_path, validate_top_level_name,
};
use crate::{
    files::digest,
    limits::Limits,
    source::{
        Value,
        kinds::preset::{decode_preset, inventory_selector},
    },
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    str,
};

/// Resolve preset template selectors only within explicitly selected area roots.
/// C36 supplies the checked closure; this adapter confers no installation authority.
#[derive(Debug)]
pub struct AreaInventories<'a> {
    /// Catalog directory containing root presets and selected owner-local areas.
    root: &'a Path,
    /// Area names mapped to catalog-relative roots; common may use the empty root.
    areas: &'a BTreeMap<String, String>,
    /// Shared production bounds, injectable only by module tests.
    limits: Limits,
}

impl<'a> AreaInventories<'a> {
    /// Create an adapter for an explicitly selected area-name to root map.
    #[must_use]
    pub const fn new(root: &'a Path, areas: &'a BTreeMap<String, String>) -> Self {
        Self {
            root,
            areas,
            limits: Limits::PRODUCTION,
        }
    }

    /// Exercise the same aggregate guards with small fixture bounds.
    #[cfg(test)]
    pub(super) const fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

/// Strict inventory of inert files and requirements, not executable content.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    /// Must equal the selected inventory's file stem.
    name: String,
    /// Binding references to report, never execute.
    bindings: Vec<String>,
    /// Executable names to report, never install or invoke.
    tools: Vec<String>,
    /// Exact source-to-output mapping and captured source digest.
    files: Vec<InventoryFile>,
}

/// A source beneath this inventory's files directory and a project-relative output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryFile {
    /// Relative to `bootstrap/<inventory>/files/` in its selected area.
    source: String,
    /// Project-root-relative output, including dotfiles.
    output: String,
    /// Existing catalog digest spelling, `sha256:<lowercase hex>`.
    sha256: String,
}

/// One resolve call shares deduplication and hostile aggregate bounds across presets.
struct Loader<'a> {
    /// Selected adapter and immutable bounds.
    adapter: &'a AreaInventories<'a>,
    /// Each selected area/inventory is included once across all requested presets.
    selected: BTreeSet<String>,
    /// First capture of each distinct path across the complete resolve call.
    source_files: FileBytes,
    /// Sources counted across presets, manifests and assets.
    count: usize,
    /// Captured input bytes across the complete request.
    bytes: u64,
}

impl PresetPort for AreaInventories<'_> {
    fn resolve(&self, names: &[String]) -> Result<Vec<Preset>, String> {
        let mut loader = Loader {
            adapter: self,
            selected: BTreeSet::new(),
            source_files: FileBytes::new(),
            count: 0,
            bytes: 0,
        };
        names.iter().map(|name| loader.preset(name)).collect()
    }
}

impl Loader<'_> {
    /// Capture each distinct source once through the existing held-handle reader.
    fn read(&mut self, path: &str) -> Result<Vec<u8>, String> {
        if let Some(bytes) = self.source_files.get(path) {
            return Ok(bytes.clone());
        }
        self.count += 1;
        if self.count > self.adapter.limits.archive_entries {
            return Err("inventory source count exceeds limit".to_owned());
        }
        let bytes = read_preset_file(self.adapter.root, path)?;
        self.bytes += u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
        if self.bytes > self.adapter.limits.archive_total_bytes {
            return Err("inventory source bytes exceed limit".to_owned());
        }
        self.source_files.insert(path.to_owned(), bytes.clone());
        Ok(bytes)
    }

    /// Decode a preset through its registered source kind and expand only its templates.
    fn preset(&mut self, name: &str) -> Result<Preset, String> {
        validate_top_level_name(name)?;
        let path = format!("presets/{name}.toml");
        let bytes = self.read(&path)?;
        let resource = decode_preset(name, &bytes)?;
        let mut preset = Preset {
            name: name.to_owned(),
            files: FileBytes::new(),
            source_files: BTreeMap::from([(path, bytes)]),
            tools: Vec::new(),
            bindings: Vec::new(),
            source_root: Some(
                self.adapter
                    .root
                    .canonicalize()
                    .map_err(|error| error.to_string())?,
            ),
        };
        for selector in resource
            .fields
            .get("templates")
            .and_then(Value::texts)
            .unwrap_or_default()
        {
            if self.selected.insert(selector.to_owned()) {
                self.inventory(selector, &mut preset)?;
            }
        }
        Ok(preset)
    }

    /// Resolve exactly two selector names; no implicit directory discovery or fallback.
    fn inventory(&mut self, selector: &str, preset: &mut Preset) -> Result<(), String> {
        let (area, name) = inventory_selector(selector)?;
        let root = self
            .adapter
            .areas
            .get(area)
            .ok_or_else(|| format!("unselected area: {area}"))?;
        if !root.is_empty() {
            validate_source_path(root)?;
        }
        let prefix = if root.is_empty() {
            String::new()
        } else {
            format!("{root}/")
        };
        let path = format!("{prefix}bootstrap/{name}.toml");
        let bytes = self
            .read(&path)
            .map_err(|error| format!("unknown inventory: {selector}: {error}"))?;
        let text = str::from_utf8(&bytes).map_err(|error| error.to_string())?;
        let inventory: Inventory = toml::from_str(text).map_err(|error| error.to_string())?;
        if inventory.name != name {
            return Err(format!("inventory name mismatch in {path}"));
        }
        for tool in &inventory.tools {
            validate_top_level_name(tool)?;
        }
        preset.source_files.insert(path, bytes);
        preset.tools.extend(inventory.tools);
        preset.bindings.extend(inventory.bindings);
        let prefix = format!("{prefix}bootstrap/{name}/files/");
        for file in &inventory.files {
            self.file(&prefix, file, preset)?;
        }
        Ok(())
    }

    /// Capture exact bytes and refuse every output collision, even identical sources.
    fn file(
        &mut self,
        prefix: &str,
        file: &InventoryFile,
        preset: &mut Preset,
    ) -> Result<(), String> {
        validate_source_path(&file.source)?;
        validate_source_path(&file.output)?;
        let path = format!("{prefix}{}", file.source);
        let bytes = self.read(&path)?;
        if digest(&bytes) != file.sha256 {
            return Err(format!(
                "inventory digest mismatch: {path}; input changed; run preview again"
            ));
        }
        if preset
            .files
            .insert(file.output.clone(), bytes.clone())
            .is_some()
        {
            return Err(format!("inventory file collision: {}", file.output));
        }
        preset.source_files.insert(path, bytes);
        Ok(())
    }
}
