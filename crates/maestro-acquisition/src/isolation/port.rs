//! A containment adapter receives already scoped handles, never source paths.
use crate::{extraction::decode::DecodeRefusal, transport::stream::Accounting};
use maestro_kernel::artifact::Digest;
#[cfg(target_os = "linux")]
use nix::{
    fcntl::AtFlags,
    mount::{MntFlags, MsFlags},
    sched::CloneFlags,
};
#[cfg(target_os = "linux")]
use rustix::thread::{CapabilitiesSecureBits, CapabilitySets};
#[cfg(target_os = "linux")]
use std::{ffi::CString, io::Result as IoResult, path::Path, process::Command};
use std::{
    fmt::Debug,
    fs::File,
    sync::{Arc, atomic::AtomicBool},
};

/// A provisioned executable or runtime file, checked once before launch.
#[derive(Debug)]
pub struct PinnedFile {
    /// Core-opened read-only handle; adapters never reopen its original path.
    pub file: File,
    /// Approved SHA-256 of exactly these bytes.
    pub digest: Digest,
}
/// A scoped read handle; its source path is not exposed to the worker.
#[derive(Debug)]
pub struct ScopedRead {
    /// Single-component name exposed under `/input`.
    pub name: String,
    /// Core-opened read-only source handle.
    pub file: File,
}
/// An explicitly provisioned runtime/model file; no ambient library directories.
#[derive(Debug)]
pub struct RuntimeFile {
    /// Relative, normalized target path in the private filesystem view.
    pub path: String,
    /// Immutable provisioned identity.
    pub pinned: PinnedFile,
    /// Whether this particular runtime file needs executable access.
    pub executable: bool,
}
/// Inputs cannot widen the document's core-owned cumulative envelope.
#[derive(Debug)]
pub struct Launch {
    /// Exact provisioned native executable, not a shell/interpreter script.
    pub parser: PinnedFile,
    /// Explicit read-only dynamic loader, library and model closure.
    pub runtime: Vec<RuntimeFile>,
    /// Already scoped inputs, never ambient credentials or grant handles.
    pub inputs: Vec<ScopedRead>,
    /// Explicit arguments; no inherited environment, proxy or PATH.
    pub arguments: Vec<String>,
    /// Required ceiling, minimum two for PID init and parser; no Pi equivalent.
    pub pids_max: u64,
    /// Core-owned cancellation shared with the caller.
    pub cancelled: Arc<AtomicBool>,
}
/// Refusals are content-free; no partial output accompanies an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Invalid handles, missing mode or invalid explicit bounds.
    Configuration,
    /// Exact provisioned launch bytes or protected bootstrap metadata changed.
    LaunchPin,
    /// A required mechanism/setup is absent; run the dedicated qualification.
    Unsupported,
    /// Host setup or a containment bootstrap failed before completion.
    Containment,
    /// Parser protocol/output ceiling refused; output is held.
    Output,
    /// Shared cumulative decode accounting refused preflight or completion.
    Decode(DecodeRefusal),
    /// The owning cancellation killed and reaped the complete tree.
    Cancelled,
    /// The anchored document deadline expired.
    Timeout,
    /// Parser crashed or returned an unsuccessful status.
    Crash,
    /// Kernel memory.events proves the owned group was OOM-killed.
    Memory,
    /// Owned cleanup failed; no result may be promoted.
    Cleanup,
}
/// Replaceable platform isolation adapter. Plain subprocesses are not adapters.
pub trait Isolation: Debug + Send + Sync {
    /// Launch, supervise, kill/reap and clean before releasing any result.
    /// Decoder IPC requests are admitted against this same document ledger.
    /// # Errors
    /// Missing kernel controls, pin changes, cancellation, crash or budget hold.
    fn run(&self, launch: Launch, accounting: &mut Accounting) -> Result<Vec<u8>, Refusal>;
}

// Delegated cgroups exist only on Linux; public isolation remains unchanged.
#[cfg(target_os = "linux")]
/// Private effects shared by the checked driver and the real Linux leaf.
pub(super) trait CgroupIo: Debug + Send + Sync {
    /// Read one kernel observation.
    fn read(&self, path: &Path) -> IoResult<String>;
    /// Write one prepared controller value or PID.
    fn write(&self, path: &Path, value: &str) -> IoResult<()>;
    /// Create a manager/worker directory.
    fn create(&self, path: &Path) -> IoResult<()>;
    /// Remove a proven-empty owned directory.
    fn remove(&self, path: &Path) -> IoResult<()>;
    /// Check whole-tree kill capability without triggering it.
    fn probe_kill(&self, path: &Path) -> IoResult<()>;
}

// Only Linux has the required namespace and filesystem primitives.
#[cfg(target_os = "linux")]
/// Private bootstrap observations/effects; production always selects kernel operations.
pub(super) trait BootstrapIo {
    /// Close every unrelated descriptor before the next boundary.
    fn hygiene(&self, keep: &[i32]) -> Result<(), Refusal>;
    /// Observe the current `AppArmor` attachment.
    fn profile(&self) -> Result<String, Refusal>;
    /// Enter one prepared namespace set.
    fn unshare(&self, flags: CloneFlags) -> Result<(), Refusal>;
    /// Apply a prepared identity map.
    fn map(&self, path: &str, value: &str) -> Result<(), Refusal>;
    /// Observe the namespace PID.
    fn pid(&self) -> i32;
    /// Reopen the pinned descriptor read-only before hiding proc.
    fn parser(&self, descriptor: i32) -> Result<File, Refusal>;
    /// Apply the default-tested mount sequence.
    fn filesystem(&self, root: &Path, memory_bytes: u64) -> Result<(), Refusal>;
    /// Apply the strict filesystem policy.
    fn landlock(&self, root: &Path, loader: Option<&str>) -> Result<(), Refusal>;
    /// Clear capability authority.
    fn capabilities(&self) -> Result<(), Refusal>;
    /// Apply the compiled syscall policy.
    fn restrict(&self) -> Result<(), Refusal>;
    /// Replace this process with the pinned image and explicit arguments.
    fn exec(
        &self,
        parser: &File,
        arguments: &[CString],
        environment: &[CString],
        flags: AtFlags,
    ) -> Result<(), Refusal>;
    /// Wait for PID-namespace init using only the prepared descriptor/configuration.
    fn handoff(&self, command: &mut Command) -> Result<bool, Refusal>;
}

#[cfg(target_os = "linux")]
/// Prepared mount and capability operations, with no policy in the host leaf.
pub(super) trait SandboxIo {
    /// Mount exactly the prepared source, target, type, flags and options.
    fn mount(&self, operation: Mount<'_>) -> Result<(), Refusal>;
    /// Change to the prepared private root.
    fn pivot(&self, root: &Path, old: &Path) -> Result<(), Refusal>;
    /// Change to a prepared directory.
    fn chdir(&self, path: &Path) -> Result<(), Refusal>;
    /// Detach the prepared old root.
    fn unmount(&self, path: &Path, flags: MntFlags) -> Result<(), Refusal>;
    /// Apply exactly the prepared securebits.
    fn securebits(&self, bits: CapabilitiesSecureBits) -> Result<(), Refusal>;
    /// Remove ambient capabilities.
    fn clear_ambient(&self) -> Result<(), Refusal>;
    /// Apply exactly the prepared capability sets.
    fn capabilities(&self, sets: CapabilitySets) -> Result<(), Refusal>;
    /// Observe whether host management remains visible.
    fn read(&self, path: &Path) -> IoResult<Vec<u8>>;
}

#[cfg(target_os = "linux")]
/// One fully prepared mount; the adapter never chooses paths, options or flags.
pub(super) struct Mount<'a> {
    /// Optional bind/device source.
    pub source: Option<&'a Path>,
    /// Explicit mount point.
    pub target: &'a Path,
    /// Optional filesystem type.
    pub filesystem: Option<&'a str>,
    /// Required security flags.
    pub flags: MsFlags,
    /// Optional already bounded tmpfs options.
    pub options: Option<&'a str>,
}
