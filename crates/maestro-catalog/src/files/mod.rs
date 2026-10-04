//! Shared digest-bound file plans, durable ownership, and crash-safe removal.
/// Exclusive writes and the final ownership commit.
mod apply;
/// Checked held-handle effects shared by owned-file adapters.
mod effects;
/// Portable names for ID-derived state records.
mod names;
/// Immutable previews and hostile relative-path validation.
mod plan;
mod publication;
/// Durable journals and restart recovery.
mod recovery;
/// Digest-checked removal of committed ownership.
mod remove;
mod replacement;
/// Contract tests for owned-file operations.
#[cfg(test)]
pub(crate) mod tests;
mod transition;

pub use crate::file_input::FileInput;
pub use apply::{apply, recover};
pub use plan::FilePlan;
pub use remove::remove;

#[cfg(test)]
pub(crate) use apply::apply_with_failure;
pub(crate) use plan::digest;
