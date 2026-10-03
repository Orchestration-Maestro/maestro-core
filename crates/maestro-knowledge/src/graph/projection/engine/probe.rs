//! Guarded read-only graph health bridge, bypassing the native handle registry.

use super::{config::native, open::open, rows};
use crate::graph::projection::{
    EngineSettings, content,
    health::{OpenGraph, ProbeError, PublishedFile, Receipt},
    port::ProjectionScope,
    receipts::{KernelProjectionReceipts, ProjectionReceiptInventory},
    writer::receipt_from_verification,
};
use lbug::{Connection, Database, RootDirectory, SystemConfig, Value};
use maestro_canonicalization::{ControlFile, ControlHandle, FileLock, LockMode, OwnedRoot};
use maestro_kernel::{
    facts::{InventoryState, ProjectionInventory, ProjectionReceipt},
    scope::{Config, LOCAL},
};
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

/// A health inventory held under permanent shared access and writer guards.
#[derive(Debug)]
pub enum ProbeReceipt {
    /// No visible current graph is published.
    NonePublished,
    /// Current receipt-bound immutable files, in inventory order.
    Files(Vec<ProbeFile>),
}

impl ProbeReceipt {
    /// Convert owned native files to the application health ports, retaining guards.
    #[must_use]
    pub fn into_receipt(self) -> Receipt<'static> {
        match self {
            Self::NonePublished => Receipt::NonePublished,
            Self::Files(files) => Receipt::Files(
                files
                    .into_iter()
                    .map(|file| Box::new(file) as Box<dyn PublishedFile>)
                    .collect(),
            ),
        }
    }
}

/// Both lock domains and the native root survive every file and open handle.
#[derive(Debug)]
struct HeldRoot {
    /// Every engine operation stays rooted in this held capability.
    native: RootDirectory,
    /// Metadata checks use the same no-follow facade under the held guards.
    owned: OwnedRoot,
    /// All three resource controls come from explicit approved caller settings.
    config: SystemConfig,
    /// Cleanup cannot enter while inventory or any native handle is alive.
    _access: ControlHandle,
    /// Detect active writers without depending on native reader lock semantics.
    _writer: ControlHandle,
}

/// One receipt-bound file; native handles are never cached by health.
#[derive(Debug)]
pub struct ProbeFile {
    /// Validated application scope, independent of native row IDs.
    scope: ProjectionScope,
    /// Immutable kernel readiness receipt.
    receipt: ProjectionReceipt,
    /// Application diagnostic location, not an unrestricted native opener.
    path: PathBuf,
    /// Keeps the guard alive between the two independent opens.
    held: Arc<HeldRoot>,
}

/// One independently constructed native read-only handle.
#[derive(Debug)]
pub(super) struct ProbeOpen {
    /// Dropped before the guards, with no connection retained outside a query.
    database: Database,
    /// Keeps cleanup excluded even if the published file wrapper is dropped.
    file: ProbeFile,
}

/// Inventory and native-file bridge for the application's existing health ports.
#[derive(Debug)]
pub struct Probe;
impl Probe {
    /// Resolve receipts only after both permanent nonblocking shared try-locks.
    /// `None` means activation is unavailable, never implicit library defaults.
    /// Problems requiring no native open and empty inventories are still reported.
    ///
    /// # Errors
    /// Refuses unavailable authority, controls and locking support. Never creates,
    /// migrates, applies grants, repairs or constructs a writable native database.
    pub fn receipt(
        data: &Path,
        directory: &Path,
        config: &Config,
        locks: &dyn FileLock,
        settings: Option<EngineSettings>,
    ) -> Result<ProbeReceipt, ProbeError> {
        let inventory = KernelProjectionReceipts::configured(data, config);
        Self::receipt_with(directory, locks, &inventory, settings)
    }

    /// The same guard-first flow with a replaceable read-only receipt inventory.
    pub(super) fn receipt_with(
        directory: &Path,
        locks: &dyn FileLock,
        inventory: &dyn ProjectionReceiptInventory,
        settings: Option<EngineSettings>,
    ) -> Result<ProbeReceipt, ProbeError> {
        let root = OwnedRoot::open(directory, false).map_err(|_| ProbeError::GuardUnavailable)?;
        let access = acquire(&root, ControlFile::Access, locks)?;
        let writer = acquire(&root, ControlFile::Writer, locks)?;
        let entries = match inventory
            .inventory(LOCAL)
            .map_err(|_| ProbeError::InventoryUnreadable)?
        {
            InventoryState::Missing => return Err(ProbeError::AuthorityMissing),
            InventoryState::NeedsMigration(_) => return Err(ProbeError::NeedsMigration),
            InventoryState::NewerSchema(_) => return Err(ProbeError::NewerSchema),
            InventoryState::Ready(entries) => entries,
        };
        if entries.is_empty() {
            return Ok(ProbeReceipt::NonePublished);
        }
        let receipts = entries
            .into_iter()
            .map(validate_receipt)
            .collect::<Result<Vec<_>, _>>()?;
        for (_, receipt) in &receipts {
            check_file(&root, &receipt.file_name)?;
        }
        let settings = settings.ok_or(ProbeError::NotActivated)?;
        let config = native(&settings).read_only(true);
        let held = Arc::new(HeldRoot {
            native: RootDirectory::open(directory)
                .map_err(|error| ProbeError::Unreadable(error.to_string()))?,
            config,
            owned: root,
            _access: access,
            _writer: writer,
        });
        Ok(ProbeReceipt::Files(
            receipts
                .into_iter()
                .map(|(scope, receipt)| ProbeFile {
                    path: directory.join(&receipt.file_name),
                    receipt,
                    scope,
                    held: Arc::clone(&held),
                })
                .collect(),
        ))
    }
}

/// Match the inventory pin and frozen canonical name before any native file open.
fn validate_receipt(
    entry: ProjectionInventory,
) -> Result<(ProjectionScope, ProjectionReceipt), ProbeError> {
    let receipt = entry.receipt.ok_or(ProbeError::MissingReceipt)?;
    let scope = ProjectionScope {
        collection_id: entry.collection_id,
        generation_id: entry.generation_id,
    };
    if receipt.collection_id != scope.collection_id
        || receipt.generation_id != scope.generation_id
        || content::basename(&scope, &receipt.claim_set_id).map_err(|_| ProbeError::Stale)?
            != receipt.file_name
    {
        return Err(ProbeError::Stale);
    }
    Ok((scope, receipt))
}

/// Distinguish the OS's typed guard contention from Unsupported and other failures.
fn acquire(
    root: &OwnedRoot,
    control: ControlFile,
    locks: &dyn FileLock,
) -> Result<ControlHandle, ProbeError> {
    let guard = root
        .open_control(control)
        .map_err(|_| ProbeError::GuardUnavailable)?;
    guard
        .lock_with(locks, LockMode::Shared, false)
        .map_err(|error| {
            if error.kind() == ErrorKind::WouldBlock {
                match control {
                    ControlFile::Access => ProbeError::CleanupInProgress,
                    ControlFile::Writer => ProbeError::Locked(error.to_string()),
                }
            } else {
                ProbeError::LockUnavailable
            }
        })?;
    Ok(guard)
}

impl ProbeFile {
    /// Construct a new read-only native database, never entering the reader registry.
    ///
    /// # Errors
    /// Refuses absent, unsafe or unreadable files; only observed native lock forms
    /// are typed as locked. Unknown native diagnostics remain unreadable.
    pub(super) fn open_native(&self) -> Result<ProbeOpen, ProbeError> {
        check_file(&self.held.owned, &self.receipt.file_name)?;
        let database = open(
            &self.held.native,
            &self.receipt.file_name,
            self.held.config.clone(),
        )
        .map_err(|error| {
            classify_native(
                &error.to_string(),
                &self.receipt.file_name,
                NativePlatform::current(),
            )
        })?;
        Ok(ProbeOpen {
            database,
            file: Self {
                scope: self.scope.clone(),
                receipt: self.receipt.clone(),
                path: self.path.clone(),
                held: Arc::clone(&self.held),
            },
        })
    }
}

impl OpenGraph for ProbeOpen {
    /// `RETURN 1` plus exact schema, row/count/digest and receipt verification.
    /// The connection and results drop on return; the database drops with this open.
    ///
    /// # Errors
    /// Reports corrupt query/schema/rows separately from stale receipt content.
    fn query_one(&self) -> Result<(), ProbeError> {
        let connection = Connection::new(&self.database)
            .map_err(|error| ProbeError::Corrupt(error.to_string()))?;
        {
            let mut result = connection
                .query("RETURN 1")
                .map_err(|error| ProbeError::Corrupt(error.to_string()))?;
            if result.next() != Some(vec![Value::Int64(1)]) || result.next().is_some() {
                return Err(ProbeError::Corrupt("unexpected RETURN 1 result".into()));
            }
        }
        let verification = rows::read(&connection, &self.file.scope)
            .and_then(|rows| rows.verification())
            .map_err(ProbeError::Corrupt)?;
        let mapped = receipt_from_verification(
            &self.file.scope,
            self.file.receipt.claim_set_id.clone(),
            self.file.receipt.file_name.clone(),
            &verification,
        )
        .map_err(|_| ProbeError::Stale)?;
        if mapped != self.file.receipt {
            return Err(ProbeError::Stale);
        }
        Ok(())
    }
}

/// Exact OS partitions for observed evidence, never a guessed POSIX equivalent.
#[derive(Debug, Clone, Copy)]
pub(super) enum NativePlatform {
    /// Captured native Windows error 33.
    Windows,
    /// Captured PID diagnostic at pin 02d90e7 on Linux.
    Linux,
    /// No approved native lock form, including macOS until CI capture.
    Other,
}
impl NativePlatform {
    /// Select only this running target's approved forms.
    pub(super) fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

/// Exact observed diagnostics only; target and full suffix must match.
pub(super) fn classify_native(message: &str, target: &str, platform: NativePlatform) -> ProbeError {
    let locked = match platform {
        NativePlatform::Windows => {
            message
                == format!(
                    "IO exception: Could not set lock on file : {target} (Error: \
                        33)\nSee the docs: https://docs.ladybugdb.com/concurrency for \
                        more information."
                )
        }
        NativePlatform::Linux => linux_locked(message, target),
        NativePlatform::Other => false,
    };
    if locked {
        ProbeError::Locked(message.to_owned())
    } else {
        ProbeError::Unreadable(message.to_owned())
    }
}

/// Captured Linux form: the pinned fork emits a canonical positive decimal PID.
fn linux_locked(message: &str, target: &str) -> bool {
    let prefix =
        format!("IO exception: Could not set lock on file : {target} (Lock is held by PID ");
    let Some(rest) = message.strip_prefix(&prefix) else {
        return false;
    };
    let Some(pid) = rest.strip_suffix(
        ")\nSee the docs: https://docs.ladybugdb.com/concurrency for more information.",
    ) else {
        return false;
    };
    let Ok(number) = pid.parse::<u32>() else {
        return false;
    };
    number > 0 && number.to_string() == pid
}

/// Diagnose missing and unsafe receipt children without native construction.
fn check_file(root: &OwnedRoot, name: &str) -> Result<(), ProbeError> {
    root.check_regular(name).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            ProbeError::MissingFile
        } else {
            ProbeError::Unreadable(error.to_string())
        }
    })
}

impl PublishedFile for ProbeFile {
    fn path(&self) -> &Path {
        &self.path
    }
    fn open_read_only(&self) -> Result<Box<dyn OpenGraph + '_>, ProbeError> {
        self.open_native()
            .map(|open| Box::new(open) as Box<dyn OpenGraph>)
    }
}
