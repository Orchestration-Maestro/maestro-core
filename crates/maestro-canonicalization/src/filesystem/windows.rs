//! Windows filesystem access: held directories and open flags that never follow a link.
//! Names resolve by path, but every directory on the way is held open without
//! `FILE_SHARE_DELETE`, so none can be renamed, deleted or replaced while the store works under
//! it, and every open carries `FILE_FLAG_OPEN_REPARSE_POINT`, so a symbolic link or junction is
//! opened itself and refused, never followed. The standard library exposes these flags safely;
//! the constants are Win32's documented values. The local filesystem must support hard links;
//! directories are not flushed, which Windows does only through a writable handle (ADR-0018).
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Read},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
};

/// `FILE_SHARE_READ`: others may read the file while the handle is open.
const FILE_SHARE_READ: u32 = 0x0000_0001;
/// `FILE_SHARE_WRITE`: others may write the file while the handle is open.
const FILE_SHARE_WRITE: u32 = 0x0000_0002;
/// `FILE_FLAG_BACKUP_SEMANTICS`: the open may name a directory.
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
/// `FILE_FLAG_OPEN_REPARSE_POINT`: a reparse point is opened itself, never followed.
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
/// `FILE_ATTRIBUTE_DIRECTORY`: the handle names a directory.
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
/// `FILE_ATTRIBUTE_REPARSE_POINT`: the handle names a reparse point, a link or junction among them.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

/// A directory reached by a path whose every directory, from its anchor on, is held open.
#[derive(Debug)]
pub(crate) struct Directory {
    /// The path walked; names inside the directory are opened below it.
    path: PathBuf,
    /// The anchor and each component, which no one can rename or delete while they stay open.
    held: Vec<File>,
}

impl Directory {
    /// Open a directory one component at a time without following links, creating missing
    /// components when asked; parent traversal is refused, and a drive or share prefix is accepted
    /// only with its root. The walk starts at the path's root or, for a relative path, at the
    /// working directory.
    pub(crate) fn open(path: &Path, create: bool) -> io::Result<Self> {
        if path
            .components()
            .any(|component| component == Component::ParentDir)
        {
            return Err(io::Error::other("snapshot path contains parent traversal"));
        }
        let anchor: PathBuf = path
            .components()
            .take_while(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
            .collect();
        if !anchor.as_os_str().is_empty() && !anchor.has_root() {
            return Err(io::Error::other(
                "snapshot path has a drive prefix without its root",
            ));
        }
        let start = if anchor.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            anchor
        };
        // The anchor, like `/` or `.` on Unix, is trusted as it resolves.
        let held = hold(&start, FILE_FLAG_BACKUP_SEMANTICS)?;
        let mut directory = Self {
            path: start,
            held: vec![held],
        };
        for component in path.components() {
            if let Component::Normal(name) = component {
                directory = open_child(directory, name, create)?;
            }
        }
        Ok(directory)
    }

    /// The bytes of a regular file in the directory, never read through a link.
    pub(crate) fn read_regular(&self, name: &str) -> io::Result<Vec<u8>> {
        let mut file = open_nofollow(&self.path.join(name))?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("artifact is not a regular file"));
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Create a file the name must not already hold, link or not.
    pub(crate) fn create_new(&self, name: &str) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(self.path.join(name))
    }

    /// Link `from` as `to`, never replacing `to`. The directory is not flushed.
    pub(crate) fn link(&self, from: &str, to: &str) -> io::Result<()> {
        fs::hard_link(self.path.join(from), self.path.join(to))
    }

    /// Remove a name from the directory; a link is removed, never followed.
    pub(crate) fn remove_file(&self, name: &str) -> io::Result<()> {
        fs::remove_file(self.path.join(name))
    }
}

/// Open and hold one child directory without following a link, creating it first when asked and
/// absent.
fn open_child(mut directory: Directory, name: &OsStr, create: bool) -> io::Result<Directory> {
    let path = directory.path.join(name);
    let child = match hold_directory(&path) {
        Err(error) if create && error.kind() == ErrorKind::NotFound => {
            match fs::create_dir(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            hold_directory(&path)?
        }
        opened => opened?,
    };
    directory.path = path;
    directory.held.push(child);
    Ok(directory)
}

/// Hold a directory that is neither a link nor a junction open against renaming and deletion.
fn hold_directory(path: &Path) -> io::Result<File> {
    let held = hold(
        path,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
    )?;
    let attributes = refuse_reparse_point(&held)?;
    if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(io::Error::new(ErrorKind::NotADirectory, "not a directory"));
    }
    Ok(held)
}

/// Open a path for reading, sharing reads and writes but never deletion or renaming.
fn hold(path: &Path, flags: u32) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(flags)
        .open(path)
}

/// The handle's attributes, or an error when it names a reparse point.
fn refuse_reparse_point(file: &File) -> io::Result<u32> {
    let attributes = file.metadata()?.file_attributes();
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::other("path names a link or reparse point"));
    }
    Ok(attributes)
}

/// Open a file for reading without following a link in its last component: a reparse point there
/// is refused. A directory opens too, so callers tell it from a file by its metadata.
pub(crate) fn open_nofollow(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    refuse_reparse_point(&file)?;
    Ok(file)
}
