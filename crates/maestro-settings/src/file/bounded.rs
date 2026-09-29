//! Bounded reads of a preferences file: no further than [`MAX_FILE_BYTES`]
//! and one byte, then UTF-8.

#[cfg(unix)]
use super::unix::Directory;
#[cfg(windows)]
use super::windows::Directory;
use crate::{
    layer::{LayerError, MAX_FILE_BYTES},
    resolve::SettingsError,
};
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read as _},
    path::Path,
};

/// The text of the preferences file `path`, `None` when it does not exist,
/// read no further than [`MAX_FILE_BYTES`] and one byte. A link is
/// followed: it reads the user file, in the directory the caller trusts.
///
/// # Errors
///
/// [`SettingsError::File`] for a larger file, and [`SettingsError::Io`]
/// for one that cannot be read or is not UTF-8.
pub(super) fn read_bounded(path: &Path) -> Result<Option<String>, SettingsError> {
    read_opened(path, File::open(path))
}

/// Reads an opened file or classifies its open error.
fn read_opened(path: &Path, opened: io::Result<File>) -> Result<Option<String>, SettingsError> {
    match opened {
        Ok(file) => read_text(file, path).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(unreadable(path, &error)),
    }
}

/// The text of the regular file `name` in `directory`, whose path is
/// `path`, opened without following a link; `None` when it does not exist.
///
/// # Errors
///
/// As [`read_bounded`], a link and a file that is not regular included.
pub(super) fn read_in(
    directory: &Directory,
    name: &OsStr,
    path: &Path,
) -> Result<Option<String>, SettingsError> {
    directory
        .open_regular(name)
        .map_err(|error| unreadable(path, &error))?
        .map(|file| read_text(file, path))
        .transpose()
}

/// The text of the open file `file`, whose path is `path`, read no further
/// than [`MAX_FILE_BYTES`] and one byte.
///
/// # Errors
///
/// As [`read_bounded`].
pub(super) fn read_text(file: File, path: &Path) -> Result<String, SettingsError> {
    let mut bytes = Vec::new();
    let limit = u64::try_from(MAX_FILE_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let size = file.metadata().map(|metadata| metadata.len()).ok();
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| unreadable(path, &error))?;
    if bytes.len() > MAX_FILE_BYTES {
        let size = size
            .and_then(|size| usize::try_from(size).ok())
            .unwrap_or(bytes.len());
        return Err(SettingsError::File {
            path: path.to_path_buf(),
            error: LayerError::TooLarge(size),
        });
    }
    String::from_utf8(bytes)
        .map_err(|error| unreadable(path, &io::Error::new(io::ErrorKind::InvalidData, error)))
}

/// The refusal of `path`, which cannot be read for `error`.
pub(super) fn unreadable(path: &Path, error: &io::Error) -> SettingsError {
    SettingsError::Io {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{SettingsError, read_opened};
    use std::{io, path::Path};

    #[test]
    fn missing_file_error_is_none() {
        assert!(matches!(
            read_opened(
                Path::new("settings.toml"),
                Err(io::Error::from(io::ErrorKind::NotFound))
            ),
            Ok(None)
        ));
    }

    #[test]
    fn permission_denied_file_error_is_an_error() {
        assert!(matches!(
            read_opened(
                Path::new("settings.toml"),
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            ),
            Err(SettingsError::Io { .. })
        ));
    }
}
