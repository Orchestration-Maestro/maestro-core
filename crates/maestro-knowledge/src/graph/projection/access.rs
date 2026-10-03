//! Permanent root-wide access and writer guards; no fallback lock domain.

use super::port::ProjectionError;
use maestro_filesystem::{ControlFile, ControlHandle, FileLock, LockMode, OwnedRoot};
use std::{io, path::Path};

/// Both setup-created controls are opened once and retained for the native lifetime.
/// ponytail: root-wide lock blocks unrelated cleanup; split only for observed contention.
#[derive(Debug)]
pub(super) struct Access {
    /// Retained shared access lock, released on drop or process exit.
    _guard: ControlHandle,
    /// Validated writer control, locked exclusively only for a producer.
    writer: ControlHandle,
}

impl Access {
    /// Validate both permanent guards and take shared access before receipt lookup.
    pub(super) fn acquire(
        root: &OwnedRoot,
        adapter: &dyn FileLock,
    ) -> Result<Self, ProjectionError> {
        let access = open(root, ControlFile::Access)?;
        let writer = open(root, ControlFile::Writer)?;
        lock(&access, adapter, ControlFile::Access, LockMode::Shared)?;
        Ok(Self {
            _guard: access,
            writer,
        })
    }

    /// Serialize native builds without waiting; cancellation cannot strand a waiter.
    pub(super) fn serialize_writer(&self, adapter: &dyn FileLock) -> Result<(), ProjectionError> {
        lock(
            &self.writer,
            adapter,
            ControlFile::Writer,
            LockMode::Exclusive,
        )
    }
}

/// Never create missing roots or repair an unsafe existing entry automatically.
pub(super) fn open_root(path: &Path) -> Result<OwnedRoot, ProjectionError> {
    OwnedRoot::open(path, false).map_err(|error| open_error(path, &error))
}

/// Never create missing guards or replace a competing lock domain.
fn open(root: &OwnedRoot, control: ControlFile) -> Result<ControlHandle, ProjectionError> {
    let path = root
        .resolved_path()
        .map_err(|error| ProjectionError::Backend(error.to_string()))?
        .join(control.file_name());
    root.open_control(control)
        .map_err(|error| open_error(&path, &error))
}

/// Setup creates missing entries only; unsafe entries require an explicit preserved move.
fn open_error(path: &Path, error: &io::Error) -> ProjectionError {
    let repair = if error.kind() == io::ErrorKind::NotFound {
        "run maestro setup --yes"
    } else {
        "Maestro won't replace it; stop graph operations, move it aside (preserving it), \
         then run maestro setup --yes"
    };
    ProjectionError::Backend(format!("{}: {error}; {repair}", path.display()))
}

/// Propagate unsupported and contended OS locks, with no fallback or blind waiting.
fn lock(
    guard: &ControlHandle,
    adapter: &dyn FileLock,
    control: ControlFile,
    mode: LockMode,
) -> Result<(), ProjectionError> {
    guard.lock_with(adapter, mode, false).map_err(|error| {
        ProjectionError::Backend(format!(
            "{}: {error}; locking must be supported \
         and no conflicting operation may hold the guard",
            control.file_name()
        ))
    })
}
