//! Reader-safe, single-receipt cleanup; native engine code is never opened here.
use maestro_canonicalization::{
    ControlFile, ControlHandle, FileLock, LockMode, OwnedRoot, ReceiptFile, SystemFileLock,
};
use maestro_kernel::{
    facts::ProjectionReceipt,
    generation::GenerationState,
    job::{self, JobState, Lease, LeaseTiming},
    journal::Filter,
    scope::{ScopeSet, collection_path},
    store::Database,
};
use serde_json::json;
use std::{error::Error, fmt, io, path::Path};

/// A guarded authorized candidate; the guard lives through deletion and job completion.
#[derive(Debug)]
pub struct Cleanup {
    /// Permanent root-wide access guard.
    guard: ControlHandle,
    /// Immutable kernel receipt retained after cleanup.
    receipt: ProjectionReceipt,
    /// Held leaf identity, or validated absence.
    file: Option<ReceiptFile>,
    /// Preview cannot authorize deletion.
    apply: bool,
}

impl Cleanup {
    /// Acquire the guard before selecting any receipt. Preview shares it; apply excludes readers.
    ///
    /// # Errors
    /// Refuses unsafe guards, busy readers, unsupported locking and unauthorized/ineligible pins.
    pub fn prepare(
        database: &Database,
        principal: &str,
        path: &Path,
        generation: i64,
        apply: bool,
    ) -> Result<Self, CleanupError> {
        let scopes = database
            .visible(principal)
            .map_err(|_| CleanupError::AuthorityUnavailable)?;
        // ponytail: root-wide guard; use per-projection guards only if real contention warrants it.
        let (root, guard) = access_guard(path, apply, &SystemFileLock)?;
        let receipt = candidate(database, &scopes, generation)?;
        let file = root
            .receipt_file(&receipt.file_name)
            .map_err(|error| file_error(&error))?;
        Ok(Self {
            guard,
            receipt,
            file,
            apply,
        })
    }

    /// Exact authorized receipt selected under the guard.
    #[must_use]
    pub fn receipt(&self) -> &ProjectionReceipt {
        &self.receipt
    }

    /// Whether the authorized disposable file is present.
    #[must_use]
    pub fn present(&self) -> bool {
        self.file.is_some()
    }

    /// Recheck current authority, generation, readiness and scoped lease before unlinking.
    ///
    /// # Errors
    /// Refuses preview sessions, changed authority, wrong/lost leases and unsafe removal.
    pub fn apply(
        &self,
        database: &Database,
        principal: &str,
        lease: &mut Lease,
        timing: LeaseTiming,
    ) -> Result<CleanupOutcome, CleanupError> {
        if !self.apply {
            return Err(CleanupError::LeaseInvalid);
        }
        let scopes = database
            .visible(principal)
            .map_err(|_| CleanupError::AuthorityUnavailable)?;
        let receipt = candidate(database, &scopes, self.receipt.generation_id)?;
        if receipt != self.receipt {
            return Err(CleanupError::UnsafeFile);
        }
        validate_lease(database, &scopes, lease, &receipt)?;
        database
            .heartbeat(lease, timing.now, timing.term)
            .map_err(|_| CleanupError::LeaseInvalid)?;
        let removed = self
            .guard
            .remove_receipt_file(&receipt.file_name, self.file.as_ref())
            .map_err(|error| file_error(&error))?;
        if removed {
            Ok(CleanupOutcome::Removed)
        } else {
            Ok(CleanupOutcome::AlreadyMissing)
        }
    }
}

/// Fixed durable success reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupOutcome {
    /// One file was unlinked and the directory synced.
    Removed,
    /// The receipt-named file was already absent; the directory was synced.
    AlreadyMissing,
}

/// Fixed public refusal codes; no filesystem/native diagnostic is disclosed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupError {
    /// Unknown and unauthorized targets are indistinguishable.
    TargetUnavailable,
    /// Only Retired and Failed-with-receipt are disposable.
    Ineligible,
    /// A Failed generation without readiness has no cleanup authority.
    ReceiptMissing,
    /// Setup must create the permanent control domain.
    GuardMissing,
    /// A supported reader, writer or cleanup owns the access guard.
    Busy,
    /// Platform cannot satisfy locking or anchored durable removal.
    Unsupported,
    /// Root/control identity or permissions are unsafe.
    UnsafeRoot,
    /// Receipt basename, leaf kind or identity is unsafe.
    UnsafeFile,
    /// Kernel authority cannot be read.
    AuthorityUnavailable,
    /// The scoped cleanup lease is missing, mismatched or lost.
    LeaseInvalid,
}

/// Acquire permanent control before any target lookup; injection is shared by all modes.
pub(super) fn access_guard(
    path: &Path,
    apply: bool,
    adapter: &dyn FileLock,
) -> Result<(OwnedRoot, ControlHandle), CleanupError> {
    let root = OwnedRoot::open(path, false).map_err(|error| root_error(&error))?;
    let guard = root
        .open_control(ControlFile::Access)
        .map_err(|error| root_error(&error))?;
    let mode = if apply {
        LockMode::Exclusive
    } else {
        LockMode::Shared
    };
    guard
        .lock_with(adapter, mode, false)
        .map_err(|error| root_error(&error))?;
    Ok((root, guard))
}

/// Resolve only a visible generation and its immutable readiness under the held guard.
fn candidate(
    database: &Database,
    scopes: &ScopeSet,
    generation: i64,
) -> Result<ProjectionReceipt, CleanupError> {
    let generation = database
        .generation(scopes, generation)
        .map_err(|_| CleanupError::AuthorityUnavailable)?
        .ok_or(CleanupError::TargetUnavailable)?;
    match generation.state {
        GenerationState::Retired | GenerationState::Failed => {}
        GenerationState::Building | GenerationState::Verified | GenerationState::Published => {
            return Err(CleanupError::Ineligible);
        }
    }
    let receipt = database
        .projection_ready(scopes, generation.id)
        .map_err(|_| CleanupError::AuthorityUnavailable)?
        .ok_or(CleanupError::ReceiptMissing)?;
    if receipt.collection_id != generation.collection_id {
        return Err(CleanupError::UnsafeFile);
    }
    Ok(receipt)
}

/// Require the live exact cleanup job, scoped to the collection and selected generation.
fn validate_lease(
    database: &Database,
    scopes: &ScopeSet,
    lease: &Lease,
    receipt: &ProjectionReceipt,
) -> Result<(), CleanupError> {
    let holder = database
        .job(scopes, lease.job)
        .map_err(|_| CleanupError::AuthorityUnavailable)?
        .ok_or(CleanupError::LeaseInvalid)?;
    if holder.state != JobState::Running
        || holder.kind != "knowledge.graph.cleanup"
        || holder.scope.as_str() != collection_path(&receipt.collection_id)
        || holder.resource.as_deref() != Some(&format!("graph-cleanup:{}", receipt.generation_id))
        || holder
            .lease
            .as_ref()
            .map(|held| (held.number, &held.holder))
            != Some((lease.number, &lease.holder))
    {
        return Err(CleanupError::LeaseInvalid);
    }
    let stream = job::stream(lease.job);
    let events = database
        .events(
            scopes,
            &Filter {
                stream: &stream,
                after: 0,
                r#type: Some(job::CREATED),
            },
        )
        .map_err(|_| CleanupError::AuthorityUnavailable)?;
    if events.first().and_then(|event| event.data.get("inputs"))
        != Some(&json!({"generation": receipt.generation_id}))
    {
        return Err(CleanupError::LeaseInvalid);
    }
    Ok(())
}

/// Preserve only fixed public root/guard failure categories.
fn root_error(error: &io::Error) -> CleanupError {
    match error.kind() {
        io::ErrorKind::NotFound => CleanupError::GuardMissing,
        io::ErrorKind::WouldBlock => CleanupError::Busy,
        io::ErrorKind::Unsupported => CleanupError::Unsupported,
        _ => CleanupError::UnsafeRoot,
    }
}

/// Never let a native or local-path diagnostic escape receipt removal.
fn file_error(error: &io::Error) -> CleanupError {
    if error.kind() == io::ErrorKind::Unsupported {
        CleanupError::Unsupported
    } else {
        CleanupError::UnsafeFile
    }
}

impl CleanupOutcome {
    /// Stable reason stored in the cleanup job outcome and returned to callers.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::AlreadyMissing => "already_missing",
        }
    }
}

impl CleanupError {
    /// Stable refusal reason; unknown and unauthorized pins share one code.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::TargetUnavailable => "target_unavailable",
            Self::Ineligible => "generation_retained",
            Self::ReceiptMissing => "receipt_missing",
            Self::GuardMissing => "guard_missing",
            Self::Busy => "access_busy",
            Self::Unsupported => "unsupported",
            Self::UnsafeRoot => "unsafe_root",
            Self::UnsafeFile => "unsafe_file",
            Self::AuthorityUnavailable => "authority_unavailable",
            Self::LeaseInvalid => "lease_invalid",
        }
    }
}

impl fmt::Display for CleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TargetUnavailable => "unknown or unauthorized graph cleanup target",
            Self::Ineligible => {
                "graph generation is retained; only Retired or Failed with a receipt \
                can be cleaned"
            }
            Self::ReceiptMissing => {
                "graph generation has no readiness receipt; preserve receiptless files \
                for recovery"
            }
            Self::GuardMissing => "graph access guard is missing; run maestro setup --yes",
            Self::Busy => "graph access is busy; let readers or writers finish, then retry cleanup",
            Self::Unsupported => {
                "platform cannot safely lock or durably remove this graph file; \
                keep the file"
            }
            Self::UnsafeRoot => {
                "graph root or access guard is unsafe; restore the owned private root \
                and guards before cleanup"
            }
            Self::UnsafeFile => {
                "graph receipt file is unsafe or replaced; preserve the file \
                and report it for recovery"
            }
            Self::AuthorityUnavailable => {
                "graph cleanup authority is unavailable; check the kernel \
                before cleanup"
            }
            Self::LeaseInvalid => "graph cleanup lease is invalid or lost; retry cleanup",
        })
    }
}

impl Error for CleanupError {}
