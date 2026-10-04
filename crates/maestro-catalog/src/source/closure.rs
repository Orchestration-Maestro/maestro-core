//! The shared exact-ID forward traversal for topology and selection admission.

use super::{
    registry::Registry,
    types::{Catalog, Problems, Resource, ResourceId},
};
use std::collections::{BTreeMap, BTreeSet};

impl Catalog {
    /// The checker's exact declared, hook and implicit-preset dependency edges.
    /// The iterator keeps the checker on borrowed loaded resources, without a
    /// second cloned catalog just to compute edges.
    pub(crate) fn dependency_edges<'a>(
        resource: &Resource,
        registry: &Registry,
        resources: impl Iterator<Item = &'a Resource>,
    ) -> Vec<ResourceId> {
        let mut targets = resource.metadata.requires.clone();
        targets.extend(registry.edges(resource));
        if resource.id.kind == "preset" {
            targets.extend(
                resources
                    .filter(|resource| {
                        resource.id.kind == "standard"
                            || (resource.id.kind == "package"
                                && ["common", "core"].contains(&resource.id.name.as_str()))
                    })
                    .map(|resource| resource.id.clone()),
            );
        }
        targets.sort();
        targets.dedup();
        targets
    }
}

/// Walks declared dependencies and caller-supplied hook edges once, in ID order.
/// Missing IDs are retained as visited and reported; callers check member evidence.
pub(crate) fn closure(
    resources: &BTreeMap<&ResourceId, &Resource>,
    roots: &[ResourceId],
    extra: impl Fn(&Resource) -> Vec<ResourceId>,
    problems: &mut Problems,
) -> BTreeSet<ResourceId> {
    let mut pending = roots.to_vec();
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let Some(resource) = resources.get(&id).copied() else {
            problems.push(("requires".to_owned(), format!("{id} does not exist")));
            continue;
        };
        pending.extend(resource.metadata.requires.iter().cloned());
        pending.extend(extra(resource));
    }
    visited
}
