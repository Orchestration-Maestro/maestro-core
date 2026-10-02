//! Registered backend contracts, independent of the adapters linked by a caller.

use super::{
    registry::Registry,
    types::{Catalog, Diagnostic, Refusal, Resource, ResourceId, Value},
};
use std::collections::BTreeSet;

/// One D14 graph control, owned here until C46 reconciles the S2 descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendBound {
    /// The functional key within its role table.
    pub key: &'static str,
    /// Inclusive minimum; zero is never an automatic sentinel.
    pub min: u64,
    /// Inclusive maximum, not a disk quota.
    pub max: u64,
    /// Whether the value must also be a power of two.
    pub power_of_two: bool,
}

/// One backend role's source schema. This registers types, not build availability.
#[derive(Debug)]
pub struct BackendDescriptor {
    /// Functional role and table name under `core/backends/`.
    pub role: &'static str,
    /// Schema-known implementations, including the graph's explicit `none`.
    pub types: &'static [&'static str],
    /// Exact allowed controls; an empty list means a strict empty table.
    pub bounds: &'static [BackendBound],
}

/// The one source of D14 bounds on this branch; it deliberately supplies no defaults.
const GRAPH_BOUNDS: &[BackendBound] = &[
    BackendBound {
        key: "buffer_pool_size",
        min: 16 * 1024 * 1024,
        max: 1024 * 1024 * 1024,
        power_of_two: false,
    },
    BackendBound {
        key: "max_db_size",
        min: 16 * 1024 * 1024,
        max: 1024 * 1024 * 1024 * 1024,
        power_of_two: true,
    },
    BackendBound {
        key: "max_num_threads",
        min: 1,
        max: 64,
        power_of_two: false,
    },
];

/// Declarative registrations for the three immutable core base roles.
/// `knowledge` names S1's knowledge MCP server; launch commands are adapter-owned.
/// Machine-local Qdrant identity, dimensions, endpoint and credentials are not fields.
pub const BACKENDS: &[BackendDescriptor] = &[
    BackendDescriptor {
        role: "graphdb",
        types: &["ladybug", "none"],
        bounds: GRAPH_BOUNDS,
    },
    BackendDescriptor {
        role: "vectordb",
        types: &["qdrant"],
        bounds: &[],
    },
    BackendDescriptor {
        role: "mcp",
        types: &["knowledge"],
        bounds: &[],
    },
];

impl Catalog {
    /// Admit a runtime closure only when every core backend's active type is linked.
    /// Runtime activation must use this seam with the composition root's compiled
    /// adapter set; [`Self::selection`] remains build-independent offline admission.
    /// All core bases join the closure, so their maturity and ownership are checked.
    /// `none` is explicitly available and never an unavailable-adapter fallback.
    /// This checks data only and makes no native, process or network calls.
    ///
    /// # Errors
    /// Offline selection failures or an active type missing from `compiled`.
    pub fn runtime_selection(
        &self,
        selected: &[ResourceId],
        registry: &Registry,
        compiled: &BTreeSet<String>,
    ) -> Result<Vec<&Resource>, Refusal> {
        let mut roots = selected.to_vec();
        for backend in self
            .resources
            .iter()
            .filter(|resource| resource.id.kind == "backend")
        {
            if !roots.contains(&backend.id) {
                roots.push(backend.id.clone());
            }
        }
        let members = self.selection(&roots, registry)?;
        let mut diagnostics = Vec::new();
        for resource in members
            .iter()
            .filter(|resource| resource.id.kind == "backend")
        {
            let kind = resource.fields.get("type").and_then(Value::text);
            if kind != Some("none") && !kind.is_some_and(|kind| compiled.contains(kind)) {
                diagnostics.push(Diagnostic::new(
                    &resource.path,
                    "type",
                    "backend adapter is not compiled into this build",
                ));
            }
        }
        if diagnostics.is_empty() {
            Ok(members)
        } else {
            Err(Refusal { diagnostics })
        }
    }
}
