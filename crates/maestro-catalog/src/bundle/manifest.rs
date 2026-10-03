//! The separately versioned compiled shape. Missing runtime declarations impose
//! no constraint. Feature/tool declarations and policy placements are not yet
//! supported by source admission; their sets are empty, never guessed.

use crate::files::digest;
use crate::{
    limits::Limits,
    source::{
        Catalog, Diagnostic, Layout, Maturity, Refusal, Registry, Resource, ResourceId, Value,
        closure::closure,
    },
};
use serde::Serialize;
use std::{collections::BTreeMap, io, slice};

/// A complete compilation, validated before any caller publishes its bytes.
#[derive(Debug)]
pub struct Bundle {
    /// Sorted fixed-metadata tar stream.
    pub bytes: Vec<u8>,
    /// Algorithm-prefixed lowercase SHA-256 of the exact tar stream.
    pub digest: String,
    /// The compiled manifest also stored as canonical `bundle.json`.
    pub manifest: Manifest,
}

/// Bundle schema `maestro-bundle/1`, separate from `maestro-source/2`.
#[derive(Debug, Serialize)]
pub struct Manifest {
    /// The bundle format version.
    pub schema: &'static str,
    /// The catalog root package's declared name.
    pub id: String,
    /// The catalog root package's exact version.
    pub version: String,
    /// Explicit full source revision supplied by release CI, never inferred.
    pub source_commit: String,
    /// SHA-256 of the policy set. Until source policy placement exists, this is
    /// SHA-256 of the canonical empty set `[]`.
    pub policy_digest: String,
    /// Every source entry, including non-resource config and inert assets.
    /// `bundle.json` itself is excluded to avoid a self-referential digest.
    pub entries: BTreeMap<String, Entry>,
    /// Checked typed resources keyed by qualified ID.
    pub resources: BTreeMap<String, Member>,
    /// Exact forward closures of every descriptor-declared root.
    pub closures: BTreeMap<String, Vec<String>>,
    /// Workflow names and exact closures of resources declaring that workflow.
    pub workflows: BTreeMap<String, Vec<String>>,
    /// Each descriptor-declared area root's sorted direct requirements.
    pub entry_points: BTreeMap<String, Vec<String>>,
    /// Snapshot-declared compatibility constraints, with no defaults.
    pub requires: Requirements,
}

/// One source file's exact content identity.
#[derive(Debug, Serialize)]
pub struct Entry {
    /// SHA-256 of its bytes.
    pub digest: String,
    /// Unpadded payload bytes.
    pub bytes: u64,
}

/// The checked descriptor data and ownership for one resource.
#[derive(Debug, Serialize)]
pub struct Member {
    /// Resource kind, selected exclusively by its registered descriptor.
    pub kind: String,
    /// Primary source path.
    pub path: String,
    /// All definition files, including sidecars.
    pub files: Vec<String>,
    /// Exact inert asset inventory.
    pub data: Vec<String>,
    /// Area-derived accountable owners.
    pub owners: Vec<String>,
    /// Delegated maintainers.
    pub maintainers: Vec<String>,
    /// Checked evidence maturity.
    pub maturity: Maturity,
    /// Architecture rows served by this resource.
    pub rows: Vec<String>,
    /// Declared workflow names.
    pub workflows: Vec<String>,
    /// Direct declared and hook-derived dependency edges.
    pub requires: Vec<String>,
    /// Optional declared metadata version.
    pub version: Option<String>,
    /// Descriptor-checked fields, unchanged, including extension kinds.
    pub fields: BTreeMap<String, Value>,
}

/// Compatibility declared in checked source data.
#[derive(Debug, Default, Serialize)]
pub struct Requirements {
    /// Runtime constraints keyed by area resource ID; absent means no constraint.
    pub runtime: BTreeMap<String, String>,
    /// Required features; empty until source descriptors declare them.
    pub features: Vec<String>,
    /// Required tool contracts; empty until source descriptors declare them.
    pub tool_contracts: Vec<String>,
}

/// A bundle refusal with the relevant source path and a bounded reason.
pub(super) fn refuse(path: &str, message: impl Into<String>) -> Refusal {
    Refusal {
        diagnostics: vec![Diagnostic::new(path, "bundle", message)],
    }
}

/// Count serialized bytes without allocating output. Construction charges each
/// member/closure before retaining it, so repeated closures cannot grow unchecked.
pub(super) struct Budget {
    /// Remaining manifest payload bytes.
    remaining: u64,
}

impl Budget {
    /// A caller-supplied bound, not a second numeric default.
    pub(super) fn new(remaining: u64) -> Self {
        Self { remaining }
    }

    /// Charge a serializable part before keeping it in the manifest.
    pub(super) fn charge(&mut self, value: &impl Serialize) -> Result<(), Refusal> {
        serde_json::to_writer(self, value).map_err(|error| refuse("bundle.json", error.to_string()))
    }
}

impl io::Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = bytes.len() as u64;
        if length > self.remaining {
            return Err(io::Error::other("manifest entry bytes exceed limit"));
        }
        self.remaining -= length;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Exact root membership through the shared forward traversal.
fn members(
    catalog: &Catalog,
    roots: &[ResourceId],
    registry: &Registry,
) -> Result<Vec<String>, Refusal> {
    let resources = catalog
        .resources
        .iter()
        .map(|resource| (&resource.id, resource))
        .collect();
    let mut problems = Vec::new();
    let ids = closure(
        &resources,
        roots,
        |resource| Catalog::dependency_edges(resource, registry, catalog.resources.iter()),
        &mut problems,
    );
    if !problems.is_empty() {
        return Err(refuse("bundle.json", format!("closure: {problems:?}")));
    }
    Ok(ids.into_iter().map(|id| id.to_string()).collect())
}

/// Preserve typed descriptor data, ownership and metadata without kind branches.
fn member(catalog: &Catalog, resource: &Resource, registry: &Registry) -> Result<Member, Refusal> {
    let owner = catalog
        .ownership(resource)
        .ok_or_else(|| refuse(&resource.path, "missing checked ownership"))?;
    Ok(Member {
        kind: resource.id.kind.clone(),
        path: resource.path.clone(),
        files: resource.files.clone(),
        data: resource.data.clone(),
        owners: owner.owners.into_iter().map(str::to_owned).collect(),
        maintainers: owner.maintainers.into_iter().map(str::to_owned).collect(),
        maturity: resource.metadata.maturity,
        rows: resource.metadata.rows.clone(),
        workflows: resource.metadata.workflows.clone(),
        requires: Catalog::dependency_edges(resource, registry, catalog.resources.iter())
            .into_iter()
            .map(|id| id.to_string())
            .collect(),
        version: resource.metadata.version.clone(),
        fields: resource.fields.clone(),
    })
}

impl Manifest {
    /// Build bounded typed metadata from the checked snapshot alone.
    pub(super) fn build(
        catalog: &Catalog,
        registry: &Registry,
        limits: &Limits,
        source_commit: &str,
    ) -> Result<(Self, Budget), Refusal> {
        let root = catalog
            .resources
            .iter()
            .find(|resource| resource.id.kind == "package" && resource.path == "package.toml")
            .ok_or_else(|| refuse("package.toml", "catalog root package is required"))?;
        let mut manifest = Self {
            schema: "maestro-bundle/1",
            id: root.id.name.clone(),
            version: root
                .fields
                .get("version")
                .and_then(Value::text)
                .unwrap_or_default()
                .to_owned(),
            source_commit: source_commit.to_owned(),
            policy_digest: digest(b"[]"),
            entries: BTreeMap::new(),
            resources: BTreeMap::new(),
            closures: BTreeMap::new(),
            workflows: BTreeMap::new(),
            entry_points: BTreeMap::new(),
            requires: Requirements::default(),
        };
        let mut budget = Budget::new(limits.archive_entry_bytes);
        let mut workflows: BTreeMap<String, Vec<ResourceId>> = BTreeMap::new();
        for resource in &catalog.resources {
            let id = resource.id.to_string();
            let value = member(catalog, resource, registry)?;
            budget.charge(&value)?;
            manifest.resources.insert(id.clone(), value);
            if registry
                .kind(&resource.id.kind)
                .is_some_and(|kind| kind.descriptor.closure_root)
            {
                let closure = members(catalog, slice::from_ref(&resource.id), registry)?;
                budget.charge(&closure)?;
                manifest.closures.insert(id.clone(), closure);
            }
            let area = registry
                .kind(&resource.id.kind)
                .is_some_and(|kind| matches!(kind.descriptor.layout, Layout::Area { .. }));
            if area {
                manifest.entry_points.insert(
                    id.clone(),
                    resource
                        .metadata
                        .requires
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                );
            }
            if area && let Some(runtime) = resource.fields.get("runtime").and_then(Value::text) {
                manifest.requires.runtime.insert(id, runtime.to_owned());
            }
            for workflow in &resource.metadata.workflows {
                workflows
                    .entry(workflow.clone())
                    .or_default()
                    .push(resource.id.clone());
            }
        }
        for (workflow, roots) in workflows {
            let closure = members(catalog, &roots, registry)?;
            budget.charge(&closure)?;
            manifest.workflows.insert(workflow, closure);
        }
        for entry_points in manifest.entry_points.values_mut() {
            entry_points.sort();
        }
        Ok((manifest, budget))
    }
}
