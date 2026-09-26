//! The store: artifacts written once under their digest, read back checked.

use super::digest::Digest;
use crate::filesystem::{create_directories, new_file, sync_directory};
use std::{
    error, fmt, fs,
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
    Corrupt {
        /// The digest they are stored under.
        expected: Digest,
        /// The digest of the bytes found in its place.
        found: Digest,
    },
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
            Self::Corrupt { expected, found } => write!(
                formatter,
                "the artifact stored under sha256:{} no longer matches its digest: its bytes \
                 hash to sha256:{}",
                expected.as_str(),
                found.as_str()
            ),
            Self::Io { path, .. } => write!(formatter, "cannot access {}", path.display()),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Missing(_) | Self::Corrupt { .. } => None,
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
            sync_directory(directory, io_error)?;
            return Ok(digest);
        }
        create_directories(directory, io_error)?;
        let temporary = directory.join(format!(
            ".tmp-{}-{}",
            process::id(),
            NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = new_file()
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
        sync_directory(directory, io_error)?;
        Ok(digest)
    }

    /// The bytes stored under `digest`, checked against it. Only a regular
    /// file is read: a link could lead out of the store, and a pipe would
    /// block.
    ///
    /// # Errors
    ///
    /// [`Error::Missing`] when nothing is stored under `digest`,
    /// [`Error::Corrupt`], naming the digest of the bytes found, when they no
    /// longer match it, and [`Error::Io`] when something other than a regular
    /// file is in its place or the file cannot be read.
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
        let found = Digest::of(&bytes);
        if found == *digest {
            Ok(bytes)
        } else {
            Err(Error::Corrupt {
                expected: digest.clone(),
                found,
            })
        }
    }

    /// Removes the artifact stored under `digest`; one already gone is no
    /// error. Garbage collection alone calls it, once no row records the
    /// artifact.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when something is in its place but cannot be removed.
    pub(crate) fn remove(&self, digest: &Digest) -> Result<(), Error> {
        let path = self.path(digest);
        match fs::remove_file(&path) {
            Err(source) if source.kind() != io::ErrorKind::NotFound => Err(io_error(&path, source)),
            _ => Ok(()),
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

/// An [`Error::Io`] about `path`.
fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}
