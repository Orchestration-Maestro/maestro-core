//! A containment adapter receives already scoped handles, never source paths.
use crate::{extraction::decode::DecodeRefusal, transport::stream::Accounting};
use maestro_kernel::artifact::Digest;
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
