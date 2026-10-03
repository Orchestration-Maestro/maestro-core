//! Linux bootstrap effects only; decisions and prepared values stay in bootstrap.
use super::{
    bootstrap::{dispatch, hygiene, pinned_parser, profile_required},
    port::{BootstrapIo, Refusal},
    sandbox,
    sandbox_host::Host,
    syscalls,
};
use nix::{
    fcntl::AtFlags,
    sched::{CloneFlags, unshare},
    unistd::execveat,
};
use rustix::process::getpid;
use std::{
    env,
    ffi::CString,
    fs::{self, File},
    io,
    path::Path,
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

/// Mandatory production adapter; only the actual OS operations live in the host leaves.
struct Effects;
impl BootstrapIo for Effects {
    fn hygiene(&self, keep: &[i32]) -> Result<(), Refusal> {
        hygiene(keep)
    }
    fn profile(&self) -> Result<String, Refusal> {
        fs::read_to_string("/proc/self/attr/current").map_err(|_| Refusal::Unsupported)
    }
    fn unshare(&self, flags: CloneFlags) -> Result<(), Refusal> {
        enter(flags)
    }
    fn map(&self, path: &str, value: &str) -> Result<(), Refusal> {
        map(path, value)
    }
    fn pid(&self) -> i32 {
        getpid().as_raw_nonzero().get()
    }
    fn parser(&self, descriptor: i32) -> Result<File, Refusal> {
        pinned_parser(descriptor)
    }
    fn filesystem(&self, root: &Path, memory_bytes: u64) -> Result<(), Refusal> {
        sandbox::filesystem_with(root, memory_bytes, &Host)
    }
    fn landlock(&self, root: &Path, loader: Option<&str>) -> Result<(), Refusal> {
        sandbox::landlock(root, loader)
    }
    fn capabilities(&self) -> Result<(), Refusal> {
        sandbox::capabilities_with(&Host)
    }
    fn restrict(&self) -> Result<(), Refusal> {
        syscalls::restrict()
    }
    fn exec(
        &self,
        parser: &File,
        arguments: &[CString],
        environment: &[CString],
        flags: AtFlags,
    ) -> Result<(), Refusal> {
        execute(parser, arguments, environment, flags)
    }
    fn handoff(&self, command: &mut Command) -> Result<bool, Refusal> {
        handoff(command)
    }
}

/// Capture only at the actual entrypoint; tests exercise this same bounded dispatcher.
/// # Errors
/// Every namespace, hard Landlock, seccomp or pinned-image failure refuses.
pub fn run(output: &mut impl io::Write, diagnostics: &mut impl io::Write) -> Result<(), Refusal> {
    dispatch(
        env::args().skip(1),
        &mut io::stdin().lock(),
        (output, diagnostics),
        profile_required(env::var("MAESTRO_N17_PROFILE").ok().as_deref()),
        &Effects,
    )
}
