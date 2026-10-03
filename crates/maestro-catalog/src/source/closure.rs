//! The shared exact-ID forward traversal for topology and selection admission.

use super::types::{Problems, Resource, ResourceId};
use std::collections::{BTreeMap, BTreeSet};

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
