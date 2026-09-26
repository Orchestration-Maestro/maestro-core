//! Where an import reads a source's corpus: its manifest, from the first
//! line, and each document a line names, relative to the manifest's own
//! directory.

use crate::RelativePath;
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

/// A source's corpus as an import reads it: the manifest, one line at a
/// time, and the document of each line, one at a time.
pub(super) trait Corpus {
    /// The manifest, from its first line.
    fn manifest(&self) -> io::Result<impl BufRead>;

    /// The bytes of the document at `path`, relative to the manifest's
    /// directory.
    fn document(&self, path: &RelativePath) -> io::Result<Vec<u8>>;
}

/// A corpus on the file system: a manifest file, and the documents below
/// its directory.
#[derive(Debug)]
pub(super) struct Files {
    /// The manifest.
    manifest: PathBuf,
    /// The manifest's directory, which the paths of its lines are relative
    /// to.
    directory: PathBuf,
}

impl Files {
    /// The corpus of the manifest `manifest`.
    pub(super) fn new(manifest: PathBuf) -> Self {
        let directory = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
        Self {
            manifest,
            directory,
        }
    }
}

impl Corpus for Files {
    fn manifest(&self) -> io::Result<impl BufRead> {
        File::open(&self.manifest).map(BufReader::new)
    }

    fn document(&self, path: &RelativePath) -> io::Result<Vec<u8>> {
        fs::read(path.under(&self.directory))
    }
}
