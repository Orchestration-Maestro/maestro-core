//! Registered backend contracts, independent of the adapters linked by a caller.

use super::{
    registry::Registry,
    types::{Catalog, Diagnostic, Refusal, Resource, ResourceId, Value},
};
use std::collections::BTreeSet;

/// One backend role's source schema. This registers types, not build availability.
#[derive(Debug)]
pub struct BackendDescriptor {
    /// Functional role and table name under `core/backends/`.
    pub role: &'static str,
    /// Schema-known implementations, including the graph's explicit `none`.
    pub types: &'static [&'static str],
}

/// Declarative registrations for the three immutable core base roles.
/// `knowledge` names S1's knowledge MCP server; launch commands are adapter-owned.
/// Machine-local Qdrant identity, dimensions, endpoint and credentials are not fields.
pub const BACKENDS: &[BackendDescriptor] = &[
    BackendDescriptor {
        role: "graphdb",
        types: &["ladybug", "none"],
    },
    BackendDescriptor {
        role: "vectordb",
        types: &["qdrant"],
    },
    BackendDescriptor {
        role: "mcp",
        types: &["knowledge"],
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
