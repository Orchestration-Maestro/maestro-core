//! The store: artifacts written once under their digest, read back checked.

use super::digest::Digest;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::{
    error, fmt,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Write as _},
    path::{self, Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

/// Numbers the temporary files of this process, so two writes never share one.
pub(super) static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// Why an artifact could not be stored or read.
#[derive(Debug)]
pub enum Error {
    /// No artifact is stored under the digest.
    Missing(Digest),
    /// The stored bytes no longer hash to their digest.
    Corrupt(Digest),
    /// A file-system operation failed; the error's source says why.
    Io {
        /// The file or directory it concerned.
        path: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(digest) => write!(
                formatter,
                "no artifact is stored under sha256:{}",
                digest.as_str()
            ),
            Self::Corrupt(digest) => write!(
                formatter,
                "the artifact stored under sha256:{} no longer matches its digest",
                digest.as_str()
            ),
            Self::Io { path, .. } => write!(formatter, "cannot access {}", path.display()),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Missing(_) | Self::Corrupt(_) => None,
        }
    }
}

/// The artifacts stored under one root directory.
#[derive(Debug, Clone)]
pub struct Store {
    /// The directory holding `sha256/`.
    root: PathBuf,
}

impl Store {
    /// The store rooted at `root`, a relative root being resolved against the
    /// current directory now; directories are created by the first write.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            root: path::absolute(&root).unwrap_or(root),
        }
    }

    /// Stores `bytes` and returns their digest. Bytes already stored intact are
    /// not written again; a stored copy that no longer matches its digest, or
    /// cannot be read as a regular file, is replaced.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when a directory or file cannot be created, written,
    /// flushed or renamed, including when the temporary file's name is taken.
    pub fn put(&self, bytes: &[u8]) -> Result<Digest, Error> {
        let digest = Digest::of(bytes);
        let path = self.path(&digest);
        let directory = path.parent().unwrap_or(&self.root);
        if self.get(&digest).is_ok() {
            // A writer of the same bytes may have renamed its copy into place
            // without flushing the directory yet.
            sync_directory(directory)?;
            return Ok(digest);
        }
        create_directories(directory)?;
        let temporary = directory.join(format!(
            ".tmp-{}-{}",
            process::id(),
            NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = temporary_options()
            .open(&temporary)
            .map_err(|source| io_error(&temporary, source))?;
        let written = file.write_all(bytes).and_then(|()| file.sync_all());
        // Closed before the rename, which Windows refuses on a file held open.
        drop(file);
        let placed = written
            .map_err(|source| io_error(&temporary, source))
            .and_then(|()| fs::rename(&temporary, &path).map_err(|source| io_error(&path, source)));
        if let Err(error) = placed {
            // This call created the temporary file, so it removes it; if that
            // fails too, it stays as a `.tmp-` file nothing refers to.
            drop(fs::remove_file(&temporary));
            return Err(error);
        }
        sync_directory(directory)?;
        Ok(digest)
    }

    /// The bytes stored under `digest`, checked against it. Only a regular
    /// file is read: a link could lead out of the store, and a pipe would
    /// block.
    ///
    /// # Errors
    ///
    /// [`Error::Missing`] when nothing is stored under `digest`,
    /// [`Error::Corrupt`] when the stored bytes no longer match it, and
    /// [`Error::Io`] when something other than a regular file is in its place
    /// or the file cannot be read.
    pub fn get(&self, digest: &Digest) -> Result<Vec<u8>, Error> {
        let path = self.path(digest);
        let failed = |source: io::Error| {
            if source.kind() == io::ErrorKind::NotFound {
                Error::Missing(digest.clone())
            } else {
                io_error(&path, source)
            }
        };
        if !fs::symlink_metadata(&path).map_err(&failed)?.is_file() {
            return Err(io_error(&path, io::Error::other("not a regular file")));
        }
        let bytes = fs::read(&path).map_err(failed)?;
        if Digest::of(&bytes) == *digest {
            Ok(bytes)
        } else {
            Err(Error::Corrupt(digest.clone()))
        }
    }

    /// Where `digest` is stored: `sha256/<2 hex>/<2 hex>/<64 hex>`.
    fn path(&self, digest: &Digest) -> PathBuf {
        let hex = digest.as_str();
        let (first, rest) = hex.split_at_checked(2).unwrap_or_default();
        let (second, _) = rest.split_at_checked(2).unwrap_or_default();
        self.root.join("sha256").join(first).join(second).join(hex)
    }
}

/// Creates `directory` and its missing ancestors for the owner only, each
/// flushed into its parent so that it survives a crash.
fn create_directories(directory: &Path) -> Result<(), Error> {
    let missing: Vec<&Path> = directory
        .ancestors()
        .take_while(|ancestor| !ancestor.is_dir())
        .collect();
    for created in missing.into_iter().rev() {
        match directory_builder().create(created) {
            Ok(()) => sync_directory(created.parent().unwrap_or(created))?,
            // Another writer made it first; a file in its place fails the
            // next step, which names it.
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(io_error(created, source)),
        }
    }
    Ok(())
}

/// Options that create a temporary file, never open an existing one: the
/// owner's only on Linux and macOS; on Windows it takes its directory's
/// access list.
fn temporary_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options
}

/// A builder of the store's directories: the owner's only on Linux and
/// macOS; on Windows each takes its parent's access list.
fn directory_builder() -> DirBuilder {
    #[cfg_attr(not(unix), expect(unused_mut, reason = "only Unix sets a mode"))]
    let mut builder = DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    builder
}

/// Flushes the entries of `directory` to the disk; on Windows, which cannot
/// flush a directory through a handle the standard library opens, nothing.
fn sync_directory(directory: &Path) -> Result<(), Error> {
    if cfg!(windows) {
        return Ok(());
    }
    File::open(directory)
        .and_then(|handle| handle.sync_all())
        .map_err(|source| io_error(directory, source))
}

/// An [`Error::Io`] about `path`.
fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}
