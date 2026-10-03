//! Where a session finds its project file (plan D6): the nearest
//! `.maestro/config.toml` upward from a directory, the command line's
//! working directory or the MCP server's `--workspace`, up to the home
//! directory inclusive and never above it. Outside home no project file is
//! read until S3's workspace trust exists (C05h), nor from a start that is
//! not a directory. A `.maestro/` or a file
//! that is a link, or a file that is not a regular one, is skipped with a
//! warning and the walk goes on; a `.maestro/` without the file is passed.
//! The owner and permission checks on the file's handle come with S3 (C05b).

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The directory of a project's preferences.
pub const PROJECT_DIRECTORY: &str = ".maestro";

/// A project's preferences file, in [`PROJECT_DIRECTORY`].
pub const PROJECT_FILE: &str = "config.toml";

/// What discovery found, for resolution and for `config explain`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    /// The nearest project file, if any.
    pub file: Option<PathBuf>,
    /// Why no project file could be looked for, when none could.
    pub note: Option<String>,
    /// Each candidate skipped, and why, nearest first.
    pub skipped: Vec<String>,
}

/// The nearest project file upward from `start`, within `home`.
#[must_use]
pub fn discover_project_file(start: &Path, home: Option<&Path>) -> Discovery {
    let noted = |note: String| Discovery {
        note: Some(note),
        ..Discovery::default()
    };
    let start = match start.canonicalize() {
        Ok(start) => start,
        Err(error) => return noted(format!("the directory cannot be resolved: {error}")),
    };
    if !start.is_dir() {
        return noted("the path is not a directory: no project file is read".to_owned());
    }
    let Some(home) = home.and_then(|home| home.canonicalize().ok()) else {
        return noted("no home directory is known: no project file is read".to_owned());
    };
    if !start.starts_with(&home) {
        return noted(
            "the directory is outside the home directory: no project file is read".to_owned(),
        );
    }
    discover_project_with(&start, &home, |directory| {
        candidate(directory).map(|file| file.map(|file| (file, ())))
    })
    .0
}

/// Walk a canonical directory up to an inclusive approved boundary, selecting one candidate.
///
/// The caller resolves `start` and `boundary` once, authorizes the boundary, and supplies a
/// held-handle reader. Its callback must reject mount roots, links, unsafe metadata and swaps.
/// `Ok(None)` continues, `Err` warns and continues, and `Ok(Some((path, snapshot)))` stops.
/// The returned snapshot is the callback's value: this function never reopens a selected file.
/// No candidate above `boundary`, outside it, or at a filesystem root is visited.
#[must_use]
pub fn discover_project_with<T>(
    start: &Path,
    boundary: &Path,
    mut candidate: impl FnMut(&Path) -> Result<Option<(PathBuf, T)>, String>,
) -> (Discovery, Option<T>) {
    let mut discovery = Discovery::default();
    if !start.is_absolute() || !boundary.is_absolute() || !start.starts_with(boundary) {
        discovery.note = Some("directory is outside the approved boundary".to_owned());
        return (discovery, None);
    }
    for directory in start.ancestors() {
        if directory.parent().is_none() {
            break;
        }
        match candidate(directory) {
            Ok(Some((file, snapshot))) => {
                discovery.file = Some(file);
                return (discovery, Some(snapshot));
            }
            Ok(None) => {}
            Err(skipped) => discovery.skipped.push(skipped),
        }
        if directory == boundary {
            break;
        }
    }
    (discovery, None)
}

/// The project file of `directory`: `Ok(None)` when it has none, and the
/// warning of a candidate skipped as `Err`.
fn candidate(directory: &Path) -> Result<Option<PathBuf>, String> {
    let folder = directory.join(PROJECT_DIRECTORY);
    match kind(&folder)? {
        Some(Kind::Directory) => {}
        Some(Kind::Link) => return Err(skipped(&folder, "a link is never followed")),
        Some(Kind::File) | None => return Ok(None),
    }
    let file = folder.join(PROJECT_FILE);
    match kind(&file)? {
        Some(Kind::File) => Ok(Some(file)),
        Some(Kind::Link) => Err(skipped(&file, "a link is never followed")),
        Some(Kind::Directory) => Err(skipped(&file, "not a regular file")),
        None => Ok(None),
    }
}

/// What a path is, without following a link at its end.
enum Kind {
    /// A directory.
    Directory,
    /// A symbolic link, or a Windows reparse point read as one.
    Link,
    /// A regular file.
    File,
}

/// What `path` is, `None` when it does not exist.
fn kind(path: &Path) -> Result<Option<Kind>, String> {
    kind_of(path, fs::symlink_metadata(path))
}

/// What a metadata lookup found, and an error that is not a missing path.
fn kind_of(path: &Path, metadata: io::Result<fs::Metadata>) -> Result<Option<Kind>, String> {
    match metadata {
        Ok(metadata) if metadata.file_type().is_symlink() => Ok(Some(Kind::Link)),
        Ok(metadata) if metadata.is_dir() => Ok(Some(Kind::Directory)),
        Ok(metadata) if metadata.is_file() => Ok(Some(Kind::File)),
        Ok(_) => Err(skipped(path, "not a regular file")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(skipped(path, &format!("cannot be read: {error}"))),
    }
}

/// The warning of `path` skipped for `reason`.
fn skipped(path: &Path, reason: &str) -> String {
    format!("{}: skipped: {reason}", path.display())
}

#[cfg(test)]
mod tests {
    use super::kind_of;
    use std::{io, path::Path};

    #[test]
    fn not_found_metadata_is_a_missing_path() {
        assert!(matches!(
            kind_of(
                Path::new("project/.maestro"),
                Err(io::Error::from(io::ErrorKind::NotFound))
            ),
            Ok(None)
        ));
    }

    #[test]
    fn permission_denied_metadata_is_not_a_missing_path() {
        assert!(matches!(
            kind_of(
                Path::new("project/.maestro"),
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            ),
            Err(reason) if reason.contains("cannot be read")
        ));
    }

    /// A device is neither a directory, a link nor a regular file. Unix
    /// only: Windows has no device path a test can read metadata of.
    #[cfg(unix)]
    #[test]
    fn device_metadata_is_not_a_regular_file() {
        use std::fs;

        let device = Path::new("/dev/null");
        assert!(matches!(
            kind_of(device, fs::symlink_metadata(device)),
            Err(reason) if reason.ends_with("skipped: not a regular file")
        ));
    }
}
