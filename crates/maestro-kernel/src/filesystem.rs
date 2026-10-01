//! The files and directories the kernel creates: its owner's only, and each
//! directory flushed into its parent so that it survives a crash.
//!
//! On Linux and macOS a file is `0600` and a directory `0700`; on Windows each
//! takes the access list of its directory, the user's profile. Windows cannot
//! flush a directory through a handle the standard library opens, so there the
//! flush is skipped, as ADR-0018 records for the snapshot store.

#[cfg(unix)]
use std::env;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Write as _},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

/// Unique create-new names within this process; collisions refuse safely.
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

/// A caller-owned root for processing authority, not a source-provided path.
/// Refuses links/reparse points, non-directories and, on Unix, foreign owners
/// or group/other write. Windows ACL ownership awaits ADR-0018's shared adapter.
///
/// # Errors
/// Unprotected roots or failed metadata/probe operations fail closed.
pub fn protected_root(root: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
    }
    let metadata = fs::metadata(root)?;
    if !metadata.is_dir() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.mode() & 0o022 != 0 {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        // A create-new probe belongs to the process's effective UID, not an
        // environment-supplied user name. It is outside the untrusted root.
        let probe = temporary(&env::temp_dir());
        let file = new_file().open(&probe)?;
        let owner = file.metadata().map(|value| value.uid());
        drop(file);
        fs::remove_file(&probe)?;
        sync_directory(&env::temp_dir(), |_, error| error)?;
        if metadata.uid() != owner? {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
    }
    Ok(())
}

/// Atomically replace one pointer with flushed complete bytes, then flush its
/// directory. The caller must serialize writers and protect the parent root.
/// Windows directory flushing follows the existing ADR-0018 store limitation.
///
/// # Errors
/// Creation, write, sync or rename failures; old or new complete bytes remain.
pub fn atomic_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let staged = temporary(parent);
    let mut file = new_file().open(&staged)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = written
        .and_then(|()| fs::rename(&staged, path))
        .and_then(|()| sync_directory(parent, |_, error| error));
    if result.is_err() {
        drop(fs::remove_file(&staged));
    }
    result
}

/// A bounded internal filename, never derived from configuration content.
fn temporary(parent: &Path) -> PathBuf {
    parent.join(format!(
        ".maestro-{}-{}",
        process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Options that create a file for its owner only, never open an existing one.
#[must_use]
pub fn new_file() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options
}

/// Creates `directory` and its missing ancestors for the owner only, each
/// flushed into its parent; `failed` is the error of a path that could not be
/// created or flushed.
pub(crate) fn create_directories<E>(
    directory: &Path,
    failed: fn(&Path, io::Error) -> E,
) -> Result<(), E> {
    let missing: Vec<&Path> = directory
        .ancestors()
        .take_while(|ancestor| !ancestor.is_dir())
        .collect();
    for created in missing.into_iter().rev() {
        match directory_builder().create(created) {
            Ok(()) => sync_directory(created.parent().unwrap_or(created), failed)?,
            // Another writer made it first; a file in its place fails the
            // next step, which names it.
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(failed(created, source)),
        }
    }
    Ok(())
}

/// A builder of directories for the owner only: `0700` on Linux and macOS; on
/// Windows each takes its parent's access list.
fn directory_builder() -> DirBuilder {
    #[cfg_attr(not(unix), expect(unused_mut, reason = "only Unix sets a mode"))]
    let mut builder = DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    builder
}

/// Flushes the entries of `directory` to the disk; on Windows, which cannot
/// flush a directory through a handle the standard library opens, nothing.
/// `failed` is the error when it cannot.
///
/// # Errors
/// Opening or flushing the directory failed, mapped by `failed`.
pub fn sync_directory<E>(directory: &Path, failed: fn(&Path, io::Error) -> E) -> Result<(), E> {
    if cfg!(windows) {
        return Ok(());
    }
    File::open(directory)
        .and_then(|handle| handle.sync_all())
        .map_err(|source| failed(directory, source))
}
