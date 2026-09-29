use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new(_name: &str) -> Self {
        Self(scratch_directory().unwrap())
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
