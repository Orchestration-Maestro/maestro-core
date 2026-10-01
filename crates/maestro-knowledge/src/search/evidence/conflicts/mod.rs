//! Detects explicit structured disagreements between admitted candidates.

/// Finds explicit value conflicts between admitted candidate families.
pub(super) mod detect;
/// Emits selected disagreements against their final passage numbers.
mod emit;
/// Parses the supported canonical table shapes.
mod tables;

pub(super) use emit::{ConflictEmission, emit_conflicts};
