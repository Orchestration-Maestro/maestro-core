//! Scoped discovery over the same aggregate snapshot used for parsing.

pub(super) use super::discovered::{Found, Unit};
use super::{area_walk, registry::Registry, scan::Snapshot, types::Refusal};
use crate::limits::Limits;

/// Find resources only in v4 scoped placements; legacy descriptors cannot register.
pub(super) fn walk_snapshot(
    snapshot: &Snapshot,
    registry: &Registry,
    limits: &Limits,
) -> Result<Found, Refusal> {
    area_walk::discover(snapshot, registry, limits)
}

/// Discovery-only test seam; production check uses the same snapshot for loading.
#[cfg(test)]
pub(super) fn walk(
    tree: &dyn super::tree::SourceTree,
    registry: &Registry,
    limits: &Limits,
) -> Result<Found, Refusal> {
    let snapshot = super::scan::scan(tree, limits)?;
    let mut found = walk_snapshot(&snapshot, registry, limits)?;
    found
        .diagnostics
        .extend(area_walk::unclaimed(&snapshot, &found.claimed));
    Ok(found)
}
