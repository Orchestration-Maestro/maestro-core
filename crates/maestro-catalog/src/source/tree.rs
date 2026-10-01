//! Catalog-relative reading through ADR-0018 held handles. The root resolves
//! once; every component below it is opened without following links.

use crate::limits::Limits;
use maestro_filesystem::{Directory as HeldDirectory, EntryKind as HeldKind};
use std::{ffi::OsStr, io, path::Path, sync::Arc};

/// What a directory entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// A link, special file or non-UTF-8 name, never followed.
    Unsupported,
}

/// One entry of a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its name, without the directory.
    pub name: String,
    /// What it is.
    pub kind: EntryKind,
}

/// Read access to a catalog's files.
pub trait SourceTree {
    /// The entries of `directory`, sorted by name.
    ///
    /// # Errors
    /// The I/O error that stopped the listing.
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>>;

    /// List at most `limit` entries. Filesystem adapters stop before allocating
    /// an unbounded listing; in-memory adapters may use the default.
    ///
    /// # Errors
    /// A failed listing or the first entry beyond the bound.
    fn list_bounded(&self, directory: &str, limit: usize) -> io::Result<Vec<Entry>> {
        let entries = self.list(directory)?;
        if entries.len() > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "directory entry limit exceeded",
            ));
        }
        Ok(entries)
    }

    /// Read at most `max_bytes + 1` bytes, retaining the sentinel byte so the
    /// caller can diagnose a source past its bound.
    ///
    /// # Errors
    /// The I/O error that stopped the read.
    fn read(&self, file: &str, max_bytes: u64) -> io::Result<Vec<u8>>;
}

/// A catalog rooted at one held filesystem directory. Links below the root
/// are unsupported entries, and are never read or traversed.
#[derive(Debug, Clone)]
pub struct Directory {
    /// Either the held root or the error that prevented opening it.
    root: Arc<io::Result<HeldDirectory>>,
}

impl Directory {
    /// Resolve and hold `root` once. An opening error is reported by reads/listings.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: Arc::new(HeldDirectory::open(root, Path::new(""), false)),
        }
    }

    /// Hold the relative directory, rejecting platform-specific path escapes.
    fn with_directory<T>(
        &self,
        relative: &str,
        read: impl FnOnce(&HeldDirectory) -> io::Result<T>,
    ) -> io::Result<T> {
        let root = self
            .root
            .as_ref()
            .as_ref()
            .map_err(|error| io::Error::new(error.kind(), error.to_string()))?;
        let mut held = None;
        if !relative.is_empty() {
            for name in relative.split('/') {
                valid_part(name)?;
                held = Some(held.as_ref().unwrap_or(root).child(name)?);
            }
        }
        read(held.as_ref().unwrap_or(root))
    }
}

/// Refuse anything other than one portable relative path component.
fn valid_part(part: &str) -> io::Result<()> {
    if matches!(part, "" | "." | "..") || part.contains(['\\', ':']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a relative catalog path",
        ));
    }
    Ok(())
}

impl SourceTree for Directory {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        self.list_bounded(directory, Limits::PRODUCTION.archive_entries)
    }

    fn list_bounded(&self, directory: &str, limit: usize) -> io::Result<Vec<Entry>> {
        let excluded = directory.is_empty().then_some(OsStr::new(".git"));
        let mut entries: Vec<_> = self
            .with_directory(directory, |held| {
                held.list_bounded_excluding(limit, excluded)
            })?
            .into_iter()
            .map(|entry| {
                let kind = match entry.kind {
                    HeldKind::File => EntryKind::File,
                    HeldKind::Directory => EntryKind::Directory,
                    HeldKind::Link | HeldKind::Other => EntryKind::Unsupported,
                };
                match entry.name.into_string() {
                    Ok(name) => Entry { name, kind },
                    Err(name) => Entry {
                        name: name.to_string_lossy().into_owned(),
                        kind: EntryKind::Unsupported,
                    },
                }
            })
            .collect();
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn read(&self, file: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        if file.starts_with('/') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a relative catalog path",
            ));
        }
        let (parent, name) = file.rsplit_once('/').unwrap_or(("", file));
        valid_part(name)?;
        self.with_directory(parent, |held| held.read_regular_prefix(name, max_bytes))
    }
}
