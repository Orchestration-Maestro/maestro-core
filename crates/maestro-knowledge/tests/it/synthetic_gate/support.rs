//! Owns private scratch directories used by synthetic-gate tests.

use maestro_test_scratch::scratch_directory;
use std::{fs, io, path::PathBuf};

pub(super) struct TestDirectory {
    pub(super) path: PathBuf,
}

impl TestDirectory {
    pub(super) fn new() -> io::Result<Self> {
        Ok(Self {
            path: scratch_directory()?,
        })
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}
