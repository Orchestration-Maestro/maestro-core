//! Explicit area-local inventory data behind the existing preset port.
use super::compose::{FileBytes, Preset, PresetPort};
use crate::{
    files::digest,
    limits::Limits,
    source::{
        Catalog, Directory, Known, Registry, Resource, ResourceId, Snapshot, SourceTree as _,
        Value,
        bootstrap_inventory::{Inventory, inventory_selector},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// Compose only a checked mandatory closure and its decoded owner-local inventories.
/// The retained bounded snapshot supplies exact preview bytes, never a second parser.
#[derive(Debug)]
pub struct AreaInventories {
    /// Canonical root, used only for later changed-input revalidation.
    root: PathBuf,
    /// The registered source kinds used for both check and selection.
    registry: Registry,
    /// Checked resource declarations, including decoded presets and inventories.
    catalog: Catalog,
    /// One bounded read of every source and inert asset.
    snapshot: Snapshot,
    /// Aggregate bounds on sources used by composition.
    limits: Limits,
}

impl AreaInventories {
    /// Check a catalog once; resolve calls admit the mandatory selected closure.
    ///
    /// # Errors
    /// Refuses invalid sources, stale generated ownership, links or source bounds.
    pub fn new(root: &Path, registry: Registry, known: Known<'_>) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let (catalog, snapshot) = Directory::new(&root)
            .checked_snapshot(&registry, &Limits::PRODUCTION, known)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            root,
            registry,
            catalog,
            snapshot,
            limits: Limits::PRODUCTION,
        })
    }

    /// Exercise resolve-wide source budgets with small fixture bounds.
    #[cfg(test)]
    pub(super) const fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

/// One resolve call shares deduplication and hostile aggregate bounds across presets.
struct Loader<'a> {
    /// Selected adapter and immutable bounds.
    adapter: &'a AreaInventories,
    /// Selected closure descriptors alone confer area admission.
    areas: BTreeMap<String, String>,
    /// Each selected area/inventory is included once across all requested presets.
    selected: BTreeSet<String>,
    /// First capture of each distinct path across the complete resolve call.
    source_files: FileBytes,
    /// Sources counted across presets, manifests and assets.
    count: usize,
    /// Captured input bytes across the complete request.
    bytes: u64,
}

impl PresetPort for AreaInventories {
    fn resolve(&self, names: &[String]) -> Result<Vec<Preset>, String> {
        let selected: Vec<_> = names
            .iter()
            .map(|name| ResourceId {
                kind: "preset".to_owned(),
                namespace: None,
                name: name.clone(),
            })
            .collect();
        let closure = self
            .catalog
            .selection(&selected, &self.registry)
            .map_err(|error| error.to_string())?;
        let areas = closure
            .iter()
            .filter(|resource| resource.fields.contains_key("owners"))
            .map(|resource| {
                (
                    resource.id.name.clone(),
                    resource
                        .path
                        .rsplit_once('/')
                        .map_or("", |(parent, _)| parent)
                        .to_owned(),
                )
            })
            .collect();
        let mut loader = Loader {
            adapter: self,
            areas,
            selected: BTreeSet::new(),
            source_files: FileBytes::new(),
            count: 0,
            bytes: 0,
        };
        closure
            .into_iter()
            .filter(|resource| resource.id.kind == "preset")
            .map(|resource| loader.preset(resource))
            .collect()
    }
}

impl Loader<'_> {
    /// Reuse each captured source once from the bounded checked snapshot.
    fn read(&mut self, path: &str) -> Result<Vec<u8>, String> {
        if let Some(bytes) = self.source_files.get(path) {
            return Ok(bytes.clone());
        }
        self.count += 1;
        if self.count > self.adapter.limits.archive_entries {
            return Err("inventory source count exceeds limit".to_owned());
        }
        let bytes = self
            .adapter
            .snapshot
            .read(path, self.adapter.limits.source_file_bytes)
            .map_err(|error| format!("cannot read {path}: {error}"))?;
        self.bytes += u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
        if self.bytes > self.adapter.limits.archive_total_bytes {
            return Err("inventory source bytes exceed limit".to_owned());
        }
        self.source_files.insert(path.to_owned(), bytes.clone());
        Ok(bytes)
    }

    /// Expand the checked closure's decoded preset templates without source parsing.
    fn preset(&mut self, resource: &Resource) -> Result<Preset, String> {
        let name = &resource.id.name;
        let path = resource.path.clone();
        let bytes = self.read(&path)?;
        let mut preset = Preset {
            name: name.to_owned(),
            files: FileBytes::new(),
            source_files: BTreeMap::from([(path, bytes)]),
            tools: Vec::new(),
            bindings: Vec::new(),
            source_root: Some(self.adapter.root.clone()),
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
            .areas
            .get(area)
            .ok_or_else(|| format!("unselected area: {area}"))?;
        let prefix = if root.is_empty() {
            String::new()
        } else {
            format!("{root}/")
        };
        let path = format!("{prefix}bootstrap/{name}.toml");
        let bytes = self
            .read(&path)
            .map_err(|error| format!("unknown inventory: {selector}: {error}"))?;
        let resource = self
            .adapter
            .catalog
            .resources
            .iter()
            .find(|resource| resource.path == path)
            .ok_or_else(|| format!("unknown inventory: {selector}"))?;
        let inventory = Inventory::decoded(resource)?;
        preset.source_files.insert(path, bytes);
        preset.tools.extend(inventory.tools);
        preset.bindings.extend(inventory.bindings);
        for (file, path) in inventory.files.iter().map(|file| {
            let path = format!("{prefix}bootstrap/{name}/files/{}", file.source);
            (file, path)
        }) {
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
        }
        Ok(())
    }
}
