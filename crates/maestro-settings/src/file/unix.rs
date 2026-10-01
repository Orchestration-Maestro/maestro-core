//! Unix: every name below the trusted directory resolves against an open
//! directory, never a path, with `O_NOFOLLOW`, so a link swapped in after a
//! check is refused rather than followed (rustix's `openat` family).

use super::place::FilePlace;
use rustix::{
    fs::{AtFlags, FileType, Mode, OFlags, mkdirat, open, openat, renameat, statat, unlinkat},
    io::Errno,
};
use std::{
    ffi::OsStr,
    fs::{self, File},
    io,
    path::Path,
};

/// Why a name is refused: a link where a file or a directory must be.
pub(super) const LINK: &str = "a link is never followed";

/// The open directory of a preferences file.
#[derive(Debug)]
pub(super) struct Directory(File);

impl Directory {
    /// The directory of `place`, creating it when `create`, else `None` if absent.
    pub(super) fn open(place: &FilePlace, create: bool) -> io::Result<Option<Self>> {
        Self::open_at(place.root(), place.below(), create)
    }

    /// Opens `root_path`, then `below` without following a link. Missing
    /// directories are created when `create`, else `None`.
    pub(super) fn open_at(
        root_path: &Path,
        below: Option<&OsStr>,
        create: bool,
    ) -> io::Result<Option<Self>> {
        if create {
            fs::create_dir_all(root_path)?;
        }
        // Each union combines distinct single-bit flags; RDONLY is zero and omitted.
        let root_flags = OFlags::DIRECTORY.union(OFlags::CLOEXEC);
        let root = match open(root_path, root_flags, Mode::empty()) {
            Ok(root) => File::from(root),
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let Some(below) = below else {
            return Ok(Some(Self(root)));
        };
        let below_flags = OFlags::DIRECTORY
            .union(OFlags::CLOEXEC)
            .union(OFlags::NOFOLLOW);
        let opened = match openat(&root, below, below_flags, Mode::empty()) {
            Err(Errno::NOENT) if create => {
                match mkdirat(&root, below, Mode::RWXU) {
                    Ok(()) | Err(Errno::EXIST) => {}
                    Err(error) => return Err(error.into()),
                }
                openat(&root, below, below_flags, Mode::empty())
            }
            opened => opened,
        };
        match opened {
            Ok(directory) => Ok(Some(Self(File::from(directory)))),
            Err(Errno::NOENT) => Ok(None),
            Err(error) => Err(refusal(&root, below, error)),
        }
    }

    /// The regular file `name`, opened for reading without following a
    /// link or blocking on a FIFO; `None` when it does not exist.
    pub(super) fn open_regular(&self, name: &OsStr) -> io::Result<Option<File>> {
        // Each `.union()` joins distinct single-bit flags.
        let flags = OFlags::NOFOLLOW
            .union(OFlags::NONBLOCK)
            .union(OFlags::CLOEXEC);
        let file = match openat(&self.0, name, flags, Mode::empty()) {
            Ok(file) => File::from(file),
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => return Err(refusal(&self.0, name, error)),
        };
        regular(file).map(Some)
    }

    /// The lock file `name`, created when absent, never through a link.
    pub(super) fn lock_file(&self, name: &OsStr) -> io::Result<File> {
        // Each `.union()` joins distinct single-bit flags.
        let flags = OFlags::RDWR
            .union(OFlags::CREATE)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC);
        openat(&self.0, name, flags, Mode::RUSR.union(Mode::WUSR))
            .map_err(|error| refusal(&self.0, name, error))
            .map(File::from)
            .and_then(regular)
    }

    /// A new file `name`, readable and writable by its owner alone; a name
    /// that exists, link or not, is refused.
    pub(super) fn create_private(&self, name: &OsStr) -> io::Result<File> {
        // Each `.union()` joins distinct single-bit flags.
        let flags = OFlags::WRONLY
            .union(OFlags::CREATE)
            .union(OFlags::EXCL)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC);
        Ok(File::from(openat(
            &self.0,
            name,
            flags,
            Mode::RUSR.union(Mode::WUSR),
        )?))
    }

    /// Renames `from` over `to`, at once, then flushes the directory's
    /// entries. A link at `to` is replaced, never followed.
    pub(super) fn rename(&self, from: &OsStr, to: &OsStr) -> io::Result<()> {
        renameat(&self.0, from, &self.0, to)?;
        self.0.sync_all()
    }

    /// Removes the name `name`; a link is removed, never followed.
    pub(super) fn remove(&self, name: &OsStr) -> io::Result<()> {
        Ok(unlinkat(&self.0, name, AtFlags::empty())?)
    }
}

/// `file`, or an error when it is not a regular file.
fn regular(file: File) -> io::Result<File> {
    if file.metadata()?.is_file() {
        Ok(file)
    } else {
        Err(io::Error::other("not a regular file"))
    }
}

/// The error of opening `name` in `directory`: [`LINK`] when the name is a
/// link, else the system's.
fn refusal(directory: &File, name: &OsStr, error: Errno) -> io::Error {
    let link = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
        .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode).is_symlink());
    if link {
        io::Error::other(LINK)
    } else {
        error.into()
    }
}
