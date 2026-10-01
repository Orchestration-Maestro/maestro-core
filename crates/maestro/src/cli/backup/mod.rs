//! Back up and restore the kernel.

/// Command handlers and archive creation.
mod command;
/// Filesystem helpers for private, streamed copies.
mod filesystem;
/// Manifest parsing, path validation and integrity checks.
mod manifest;
/// Shared kernel-owned path names.
mod names;
/// Restore staging and the final database commit.
mod restore;
#[cfg(test)]
/// Shared scratch paths for unit tests.
mod test_support;
#[cfg(test)]
/// Interruption handling checks.
mod tests;

pub(super) use command::{run_backup, run_restore};
