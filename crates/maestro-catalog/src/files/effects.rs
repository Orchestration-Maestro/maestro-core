//! Owned-file adapters use only the shared policy's held-handle effects.
use crate::policy::workspace::{Access, CheckedTrust};
use std::{
    fs::Metadata,
    io::{self, Read as _},
    path::Path,
};

/// Read through one fresh held read capability, never a reopened pathname.
pub(super) fn read(root: &Path, relative: &str, trust: &CheckedTrust<'_>) -> io::Result<Vec<u8>> {
    let mut file = trust
        .authorize(root, Path::new(relative), Access::Read)?
        .open_read()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Create missing parents and write exactly one new leaf through the same floor.
pub(super) fn write(
    root: &Path,
    relative: &str,
    bytes: &[u8],
    trust: &CheckedTrust<'_>,
) -> io::Result<Metadata> {
    trust
        .authorize_create(root, Path::new(relative))?
        .write_new(bytes)
}

/// Fresh write authority intersects the writer's existing digest/identity ownership preconditions.
pub(super) fn remove(
    root: &Path,
    relative: &str,
    bytes: &[u8],
    identity: Option<(u64, u64)>,
    trust: &CheckedTrust<'_>,
) -> io::Result<()> {
    trust
        .authorize(root, Path::new(relative), Access::Write)?
        .remove_verified(bytes, identity)
}

/// Preflight the entire plan before the first state record or directory creation.
pub(super) fn check(root: &Path, relative: &str, trust: &CheckedTrust<'_>) -> io::Result<()> {
    trust.check_write(root, Path::new(relative))
}
