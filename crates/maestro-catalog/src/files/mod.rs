//! Shared digest-bound file plans, durable ownership, and crash-safe removal.
/// Exclusive writes and the final ownership commit.
mod apply;
/// Immutable previews and hostile relative-path validation.
mod plan;
/// Durable journals and restart recovery.
mod recovery;
/// Digest-checked removal of committed ownership.
mod remove;
/// Contract tests for owned-file operations.
#[cfg(test)]
mod tests;

pub use apply::{apply, recover};
pub use plan::{FileInput, FilePlan};
pub use remove::remove;

#[cfg(test)]
pub(crate) use apply::apply_with_failure;
pub(crate) use plan::digest;
