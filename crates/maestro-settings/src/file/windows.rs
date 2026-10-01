//! Windows: names resolve by path, but the directory of the file is held
//! open without `FILE_SHARE_DELETE`, so it cannot be renamed, deleted or
//! replaced while the edit runs, and every open carries
//! `FILE_FLAG_OPEN_REPARSE_POINT`, so a symbolic link or junction is opened
//! itself and refused, never followed. The constants are Win32's documented
//! values; the standard library exposes the flags safely.

use super::{
    place::FilePlace,
    windows_logic::{found, is_reparse_point, tolerate_existing},
};
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind},
    os::windows::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::{Path, PathBuf},
};

/// Why a name is refused: a link where a file or a directory must be.
pub(super) const LINK: &str = "a link is never followed";

/// The combined `FILE_SHARE_READ | FILE_SHARE_WRITE` value.
const FILE_SHARE_READ_WRITE: u32 = 0x0000_0003;
/// `FILE_FLAG_OPEN_REPARSE_POINT`: a reparse point is opened itself.
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
/// The combined `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT` value.
const FILE_FLAG_DIRECTORY_REPARSE: u32 = 0x0220_0000;
/// `FILE_ATTRIBUTE_REPARSE_POINT`: the handle names a link or junction.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

/// The held directory of a preferences file.
#[derive(Debug)]
pub(super) struct Directory {
    /// The directory's path; names inside it are opened below it.
    path: PathBuf,
    /// The directory itself, held against renaming and deletion.
    _held: File,
}

impl Directory {
    /// The directory of `place`: its trusted directory, then the directory
    /// below it, which must be neither a link nor a junction. Missing
    /// directories are created when `create`, else `None`.
    pub(super) fn open(place: &FilePlace, create: bool) -> io::Result<Option<Self>> {
        if create {
            create_directories(place)?;
        }
        let path = place.directory();
        let Some(held) = found(hold(&path, FILE_FLAG_DIRECTORY_REPARSE))? else {
            return Ok(None);
        };
        unlinked(&held)?;
        if !held.metadata()?.is_dir() {
            return Err(io::Error::new(ErrorKind::NotADirectory, "not a directory"));
        }
        Ok(Some(Self { path, _held: held }))
    }

    /// The settings file `name`, opened for reading without following a
    /// link; `None` when it does not exist. Callers pass fixed settings file
    /// names. Windows refuses a directory without backup semantics, and
    /// `unlinked` refuses reparse points.
    pub(super) fn open_regular(&self, name: &OsStr) -> io::Result<Option<File>> {
        let Some(file) = found(hold(&self.path.join(name), FILE_FLAG_OPEN_REPARSE_POINT))? else {
            return Ok(None);
        };
        unlinked(&file)?;
        Ok(Some(file))
    }

    /// The lock file `name`, created when absent, never through a link.
    pub(super) fn lock_file(&self, name: &OsStr) -> io::Result<File> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(self.path.join(name))?;
        unlinked(&file)?;
        Ok(file)
    }

    /// A new file `name`; a name that exists, link or not, is refused. It
    /// takes the directory's inherited access control.
    pub(super) fn create_private(&self, name: &OsStr) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(self.path.join(name))
    }

    /// Renames `from` over `to`, at once. A link at `to` is replaced, never
    /// followed. The directory is not flushed, which Windows does only
    /// through a writable handle.
    pub(super) fn rename(&self, from: &OsStr, to: &OsStr) -> io::Result<()> {
        fs::rename(self.path.join(from), self.path.join(to))
    }

    /// Removes the name `name`; a link is removed, never followed.
    pub(super) fn remove(&self, name: &OsStr) -> io::Result<()> {
        fs::remove_file(self.path.join(name))
    }
}

/// Creates the directories of `place` that are missing.
fn create_directories(place: &FilePlace) -> io::Result<()> {
    fs::create_dir_all(place.root())?;
    let Some(below) = place.below() else {
        return Ok(());
    };
    tolerate_existing(fs::create_dir(place.root().join(below)))
}

/// Opens `path` for reading with `flags`, sharing reads and writes but
/// never deletion or renaming.
fn hold(path: &Path, flags: u32) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ_WRITE)
        .custom_flags(flags)
        .open(path)
}

/// [`LINK`] when `file` names a reparse point.
fn unlinked(file: &File) -> io::Result<()> {
    if is_reparse_point(
        file.metadata()?.file_attributes(),
        FILE_ATTRIBUTE_REPARSE_POINT,
    ) {
        return Err(io::Error::other(LINK));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Directory, FilePlace, LINK};
    use std::{
        error::Error,
        ffi::OsStr,
        fs, io,
        os::windows::fs::symlink_file,
        path::{Path, PathBuf},
    };

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Result<Self, Box<dyn Error>> {
            Ok(Self(maestro_test_scratch::scratch_directory()?))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    fn directory(root: &Path) -> io::Result<Directory> {
        Directory::open(&FilePlace::user(root), false)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "test directory disappeared"))
    }

    #[test]
    fn open_regular_reports_file_symlink_as_link() -> Result<(), Box<dyn Error>> {
        let scratch = Scratch::new()?;
        fs::write(scratch.0.join("target"), "contents")?;
        symlink_file("target", scratch.0.join("linked"))?;

        let error = directory(&scratch.0)?
            .open_regular(OsStr::new("linked"))
            .expect_err("file symlink must be refused");
        assert_eq!(error.to_string(), LINK);
        Ok(())
    }

    #[test]
    fn open_regular_refuses_directories_before_opening_a_handle() -> Result<(), Box<dyn Error>> {
        let scratch = Scratch::new()?;
        fs::create_dir(scratch.0.join("directory"))?;
        assert!(
            directory(&scratch.0)?
                .open_regular(OsStr::new("directory"))
                .is_err()
        );
        Ok(())
    }
}
