//! The port through which the checker reads a catalog's files, and its
//! filesystem adapter. Paths are relative and `/`-separated; the root is `""`.

use std::{
    fs::{self, File},
    io::{self, Read as _},
    path::{Path, PathBuf},
};

/// What a directory entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// A link, a device, a name that is not UTF-8, or anything else the
    /// checker never follows.
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
    ///
    /// The I/O error that stopped the listing.
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>>;

    /// The bytes of `file`, reading at most `max_bytes + 1` of them, so a
    /// caller can tell a file at the limit from one past it without
    /// allocating for a larger one.
    ///
    /// # Errors
    ///
    /// The I/O error that stopped the read.
    fn read(&self, file: &str, max_bytes: u64) -> io::Result<Vec<u8>>;
}

/// A catalog in a directory of the filesystem. Links are listed as
/// [`EntryKind::Unsupported`] and never followed.
#[derive(Debug, Clone)]
pub struct Directory {
    /// The catalog's root.
    root: PathBuf,
}

impl Directory {
    /// The catalog rooted at `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// The filesystem path of the relative path `relative`, `""` for the
    /// root.
    ///
    /// # Errors
    ///
    /// [`io::ErrorKind::InvalidInput`] when a part is empty, `.`, `..`, or
    /// holds `\\` or `:`, which could leave or bypass the root on some
    /// platform.
    fn path(&self, relative: &str) -> io::Result<PathBuf> {
        if relative.is_empty() {
            return Ok(self.root.clone());
        }
        let mut path = self.root.clone();
        for part in relative.split('/') {
            if matches!(part, "" | "." | "..") || part.contains(['\\', ':']) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{relative:?} is not a relative catalog path"),
                ));
            }
            path.push(part);
        }
        Ok(path)
    }
}

impl SourceTree for Directory {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(self.path(directory)?)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name().into_string();
            let kind = match &name {
                Ok(_) if file_type.is_file() => EntryKind::File,
                Ok(_) if file_type.is_dir() => EntryKind::Directory,
                _ => EntryKind::Unsupported,
            };
            let name = name.unwrap_or_else(|raw| raw.to_string_lossy().into_owned());
            entries.push(Entry { name, kind });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn read(&self, file: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        File::open(self.path(file)?)?
            .take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
