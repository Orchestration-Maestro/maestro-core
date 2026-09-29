//! The file adapter of [`LayerSource`]: the user's `preferences.toml` in
//! the configuration directory, and the project's `.maestro/config.toml`,
//! whose directory and file are read without following a link.

#[cfg(unix)]
use super::unix::Directory;
#[cfg(windows)]
use super::windows::Directory;
use super::{
    bounded::{read_bounded, read_in, unreadable},
    place::FilePlace,
};
use crate::{
    layer::Layer,
    registry::Registry,
    resolve::{LayerName, Layers, SettingsError},
    store::{LayerSource, USER_FILE},
};
use std::path::{Path, PathBuf};

/// The files of a session: the user file, and the project file when
/// discovery found one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLayers {
    /// The user file.
    pub user: PathBuf,
    /// The project file, if any.
    pub project: Option<PathBuf>,
}

impl FileLayers {
    /// The user file of `config_dir` and the project file `project`.
    #[must_use]
    pub fn new(config_dir: &Path, project: Option<PathBuf>) -> Self {
        Self {
            user: config_dir.join(USER_FILE),
            project,
        }
    }

    /// The path of the file of `layer`, if the session has one.
    #[must_use]
    pub fn path_of(&self, layer: LayerName) -> Option<PathBuf> {
        match layer {
            LayerName::User => Some(self.user.clone()),
            LayerName::Project => self.project.clone(),
        }
    }
}

impl LayerSource for FileLayers {
    fn layers(&self, registry: &Registry) -> Result<Layers, SettingsError> {
        let read = |path: &Path,
                    text: Option<String>|
         -> Result<Option<(PathBuf, Layer)>, SettingsError> {
            let Some(text) = text else {
                return Ok(None);
            };
            let layer = Layer::parse(registry, &text).map_err(|error| SettingsError::File {
                path: path.to_path_buf(),
                error,
            })?;
            Ok(Some((path.to_path_buf(), layer)))
        };
        let project = match &self.project {
            Some(path) => {
                let place = FilePlace::project(path).ok_or_else(|| SettingsError::Io {
                    path: path.clone(),
                    reason: "not a file in a directory".to_owned(),
                })?;
                read(path, read_place(&place)?)?
            }
            None => None,
        };
        Ok(Layers {
            user: read(&self.user, read_bounded(&self.user)?)?,
            project,
        })
    }
}

/// The text of the file of `place`, `None` when it or its directory does
/// not exist, read without following a link below its trusted directory.
fn read_place(place: &FilePlace) -> Result<Option<String>, SettingsError> {
    let path = place.path();
    let Some(directory) =
        Directory::open(place, false).map_err(|error| unreadable(&path, &error))?
    else {
        return Ok(None);
    };
    read_in(&directory, place.name(), &path)
}
