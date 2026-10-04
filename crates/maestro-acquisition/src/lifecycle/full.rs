//! Full revalidation compares source evidence, never visible text alone.
use maestro_kernel::{
    acquisition::{CaptureEnvelope, ChangeKeys},
    artifact::Digest,
};
use serde::{Deserialize, Serialize};

/// Manual lifecycle mode; scheduling policy remains independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Revalidate every currently admitted known item and discover new items.
    Full,
    /// Verify outstanding local windows with declared uncertainty margins.
    Incremental,
}

/// Unchanged requires equal known bytes; one-sided optional knowledge is ignored.
#[must_use]
pub fn changed(previous: &ChangeKeys, current: &ChangeKeys) -> bool {
    let same_bytes = previous
        .representation
        .as_ref()
        .zip(current.representation.as_ref())
        .is_some_and(|(previous, current)| previous == current);
    !same_bytes
        || known_difference(previous.revision.as_ref(), current.revision.as_ref())
        || known_difference(previous.validator.as_ref(), current.validator.as_ref())
        || known_difference(previous.metadata.as_ref(), current.metadata.as_ref())
        || previous.permissions != current.permissions
        || known_difference(previous.links.as_ref(), current.links.as_ref())
}

/// Only two actual observations can establish a changed optional signal.
fn known_difference(previous: Option<&Digest>, current: Option<&Digest>) -> bool {
    previous
        .zip(current)
        .is_some_and(|(previous, current)| previous != current)
}

/// Compare public HTTP source evidence, excluding local run and attempt observations.
#[must_use]
pub fn same_capture(previous: &CaptureEnvelope, current: &CaptureEnvelope) -> bool {
    (
        &previous.artifact,
        &previous.headers,
        &previous.declared_media,
        &previous.detected_media,
        &previous.authorization_context,
        &previous.profile,
        &previous.final_identity,
        previous.status,
        previous.representation,
    ) == (
        &current.artifact,
        &current.headers,
        &current.declared_media,
        &current.detected_media,
        &current.authorization_context,
        &current.profile,
        &current.final_identity,
        current.status,
        current.representation,
    )
}
