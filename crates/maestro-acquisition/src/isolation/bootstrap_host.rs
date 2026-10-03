//! Linux bootstrap effects only; decisions and prepared values stay in bootstrap.
use super::port::Refusal;
use nix::{
    fcntl::AtFlags,
    sched::{CloneFlags, unshare},
    unistd::execveat,
};
use std::{
    ffi::CString,
    fs::{self, File},
    process::Command,
};

/// Enter one already selected namespace set.
pub(super) fn enter(flags: CloneFlags) -> Result<(), Refusal> {
    unshare(flags).map_err(|_| Refusal::Unsupported)
}
/// Write one already prepared UID/GID map or setgroups policy.
pub(super) fn map(path: &str, value: &str) -> Result<(), Refusal> {
    fs::write(path, value).map_err(|_| Refusal::Unsupported)
}
/// Spawn namespace init from the pinned FD, with no ambient environment/stderr.
pub(super) fn handoff(command: &mut Command) -> Result<bool, Refusal> {
    command
        .status()
        .map(|status| status.success())
        .map_err(|_| Refusal::Containment)
}
/// Exec only the pinned descriptor with the explicit argv and an empty environment.
pub(super) fn execute(
    parser: &File,
    arguments: &[CString],
    environment: &[CString],
    flags: AtFlags,
) -> Result<(), Refusal> {
    execveat(parser, c"", arguments, environment, flags).map_err(|_| Refusal::Containment)?;
    Ok(())
}
