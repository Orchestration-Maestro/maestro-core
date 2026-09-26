//! The files and directories the kernel creates: its owner's only, and each
//! directory flushed into its parent so that it survives a crash.
//!
//! On Linux and macOS a file is `0600` and a directory `0700`; on Windows each
//! takes the access list of its directory, the user's profile. Windows cannot
//! flush a directory through a handle the standard library opens, so there the
//! flush is skipped, as ADR-0018 records for the snapshot store.

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::{
    fs::{DirBuilder, File, OpenOptions},
    io,
    path::Path,
};

/// Options that create a file for its owner only, never open an existing one.
pub(crate) fn new_file() -> OpenOptions {
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
pub(crate) fn sync_directory<E>(
    directory: &Path,
    failed: fn(&Path, io::Error) -> E,
) -> Result<(), E> {
    if cfg!(windows) {
        return Ok(());
    }
    File::open(directory)
        .and_then(|handle| handle.sync_all())
        .map_err(|source| failed(directory, source))
}
