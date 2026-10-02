//! Explicit area-local inventory data behind the existing preset port.
use super::compose::{FileBytes, Preset, PresetPort, SourceFile};
use crate::{
    files::digest,
    limits::Limits,
    source::{
        BACKENDS, Catalog, DEFAULTS_PATH, Directory, Known, Registry, Resource, ResourceId,
        Snapshot, SourceTree as _, Value,
        bootstrap_inventory::{Inventory, inventory_selector},
        from_resources,
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
    /// Checked lowest defaults slot captured from the same source snapshot.
    defaults: maestro_settings::Registry,
    /// Actual adapters supplied by the composition root, never source claims.
    compiled: BTreeSet<String>,
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
        let settings = known.settings.registry().cloned().map_or_else(
            maestro_settings::Registry::built_in,
            Ok,
        ).map_err(|error| error.to_string())?;
        let defaults = from_resources(
            &settings,
            &catalog.resources,
            catalog.common_defaults.as_deref(),
            &Limits::PRODUCTION,
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            root,
            registry,
            catalog,
            snapshot,
            limits: Limits::PRODUCTION,
            defaults,
            compiled: BTreeSet::new(),
        })
    }

    /// Bind the adapters linked by the composition root; `none` needs none.
    #[must_use]
    pub fn with_compiled_backends(mut self, compiled: BTreeSet<String>) -> Self {
        self.compiled = compiled;
        self
    }

    /// The immutable defaults checked when this adapter was constructed.
    #[must_use]
    pub const fn admitted_registry(&self) -> &maestro_settings::Registry {
        &self.defaults
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
    source_files: BTreeMap<String, SourceFile>,
    /// Sources counted across presets, manifests and assets.
    count: usize,
    /// Captured input bytes across the complete request.
    bytes: u64,
}

impl PresetPort for AreaInventories {
    fn defaults(&self) -> Result<maestro_settings::Registry, String> {
        Ok(self.defaults.clone())
    }

    fn absent_inputs(&self) -> Vec<(PathBuf, String)> {
        let mut paths = vec![DEFAULTS_PATH.to_owned()];
        paths.extend(
            BACKENDS
                .iter()
                .map(|backend| format!("core/backends/{}/config.toml", backend.role)),
        );
        paths
            .into_iter()
            .filter(|path| {
                if path == DEFAULTS_PATH {
                    self.catalog.common_defaults.is_none()
                } else {
                    self.snapshot.read(path, self.limits.source_file_bytes).is_err()
                }
            })
            .map(|path| (self.root.clone(), path))
            .collect()
    }

    fn backend_types(&self) -> BTreeSet<String> {
        self.catalog
            .resources
            .iter()
            .filter(|resource| resource.id.kind == "backend")
            .filter_map(|resource| {
                resource
                    .fields
                    .get("type")
                    .and_then(Value::text)
                    .map(str::to_owned)
            })
            .collect()
    }

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
            .runtime_selection(&selected, &self.registry, &self.compiled)
            .map_err(|error| error.to_string())?;
        let area_ids = closure
            .iter()
            .filter(|resource| resource.fields.contains_key("owners"))
            .map(|resource| resource.id.to_string())
            .collect();
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
            source_files: BTreeMap::new(),
            count: 0,
            bytes: 0,
        };
        for resource in &closure {
            for path in resource.files.iter().chain(&resource.data) {
                loader.capture(resource, path)?;
            }
        }
        if self.catalog.common_defaults.is_some() {
            let common = closure
                .iter()
                .find(|resource| resource.id.to_string() == "package:common")
                .ok_or("common defaults require package:common")?;
            loader.capture(common, DEFAULTS_PATH)?;
        }
        let mut presets: Vec<_> = closure
            .into_iter()
            .filter(|resource| resource.id.kind == "preset")
            .map(|resource| loader.preset(resource))
            .collect::<Result<_, _>>()?;
        if let Some(first) = presets.first_mut() {
            first.areas = area_ids;
            first.source_files = loader.source_files;
        }
        Ok(presets)
    }
}

/// Metadata revisions take precedence; checked area versions are the fallback.
fn source_revision(resource: &Resource) -> Option<String> {
    resource.metadata.version.clone().or_else(|| {
        resource
            .fields
            .contains_key("owners")
            .then(|| resource.fields.get("version").and_then(Value::text))
            .flatten()
            .map(str::to_owned)
    })
}

impl Loader<'_> {
    /// Reuse each captured source once from the bounded checked snapshot.
    fn capture(&mut self, resource: &Resource, path: &str) -> Result<(), String> {
        if self.source_files.contains_key(path) {
            return Ok(());
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
        self.source_files.insert(
            path.to_owned(),
            SourceFile {
                id: resource.id.to_string(),
                revision: source_revision(resource),
                bytes,
            },
        );
        Ok(())
    }

    /// Expand the checked closure's decoded preset templates without source parsing.
    fn preset(&mut self, resource: &Resource) -> Result<Preset, String> {
        let name = &resource.id.name;
        let mut preset = Preset {
            name: name.to_owned(),
            files: FileBytes::new(),
            source_files: BTreeMap::new(),
            areas: Vec::new(),
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
        let resource = self
            .adapter
            .catalog
            .resources
            .iter()
            .find(|resource| resource.path == path)
            .ok_or_else(|| format!("unknown inventory: {selector}"))?;
        let inventory = Inventory::decoded(resource)?;
        preset.tools.extend(inventory.tools);
        preset.bindings.extend(inventory.bindings);
        for (file, path) in inventory.files.iter().map(|file| {
            let path = format!("{prefix}bootstrap/{name}/files/{}", file.source);
            (file, path)
        }) {
            let bytes = self
                .source_files
                .get(&path)
                .ok_or_else(|| format!("inventory input missing from selected closure: {path}"))?
                .bytes
                .clone();
            if digest(&bytes) != file.sha256 {
                return Err(format!(
                    "inventory digest mismatch: {path}; input changed; run preview again"
                ));
            }
            if preset.files.insert(file.output.clone(), bytes).is_some() {
                return Err(format!("inventory file collision: {}", file.output));
            }
        }
        Ok(())
    }
}
