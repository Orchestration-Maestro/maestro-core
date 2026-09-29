//! Where a preferences file lives: a directory the caller trusts, the one
//! directory below it that is opened without following a link, and the
//! file's name.

use crate::store::USER_FILE;
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

/// A preferences file's place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePlace {
    /// The directory the caller trusts, such as the configuration directory
    /// or a project's directory; its links resolve.
    root: PathBuf,
    /// The directory below `root` that holds the file, opened without
    /// following a link: `.maestro` for a project.
    below: Option<OsString>,
    /// The file's name, opened without following a link.
    name: OsString,
}

impl FilePlace {
    /// The user file in the configuration directory `config_dir`.
    #[must_use]
    pub fn user(config_dir: &Path) -> Self {
        Self {
            root: config_dir.to_path_buf(),
            below: None,
            name: USER_FILE.into(),
        }
    }

    /// The project file `file`, such as `<project>/.maestro/config.toml`:
    /// its directory's parent is trusted, and its directory and name are
    /// opened without following a link. `None` for a path without both.
    #[must_use]
    pub fn project(file: &Path) -> Option<Self> {
        let name = file.file_name()?;
        let directory = file.parent()?;
        Some(Self {
            root: directory.parent()?.to_path_buf(),
            below: Some(directory.file_name()?.to_os_string()),
            name: name.to_os_string(),
        })
    }

    /// The trusted directory.
    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    /// The directory below the trusted one, if any.
    pub(super) fn below(&self) -> Option<&OsStr> {
        self.below.as_deref()
    }

    /// The file's name.
    pub(super) fn name(&self) -> &OsStr {
        &self.name
    }

    /// The file's directory.
    #[must_use]
    pub fn directory(&self) -> PathBuf {
        self.below
            .as_ref()
            .map_or_else(|| self.root.clone(), |below| self.root.join(below))
    }

    /// The file's path.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.directory().join(&self.name)
    }

    /// The path of the file `suffix` names beside it, such as its lock.
    pub(super) fn beside(&self, suffix: &str) -> OsString {
        let mut name = self.name.clone();
        name.push(suffix);
        name
    }
}
