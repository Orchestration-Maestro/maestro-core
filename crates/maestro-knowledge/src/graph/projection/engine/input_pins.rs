//! Native build stamps and comparison with independently persisted readiness pins.
use crate::graph::projection::{
    build::ProjectionBuild,
    port::{InputMismatchKind, ProjectionError},
};

/// Canonical native stamp values in resolution, resolver, settings, lock order.
pub(in crate::graph::projection) fn build_pins(build: &ProjectionBuild) -> [String; 4] {
    [
        build.resolution_id.as_str().to_owned(),
        build.resolver_version.clone(),
        build.settings_identity.as_str().to_owned(),
        build.frozen_lock.as_str().to_owned(),
    ]
}

/// Compare the native durable stamp to the receipt without conflating corruption.
pub(in crate::graph::projection) fn compare(
    found: &[String; 4],
    expected: &[String; 4],
) -> Result<(), ProjectionError> {
    for (index, kind) in [
        (0, InputMismatchKind::Resolution),
        (1, InputMismatchKind::Resolution),
        (2, InputMismatchKind::Settings),
        (3, InputMismatchKind::Lock),
    ] {
        if found.get(index) != expected.get(index) {
            return Err(ProjectionError::InputMismatch(kind));
        }
    }
    Ok(())
}
