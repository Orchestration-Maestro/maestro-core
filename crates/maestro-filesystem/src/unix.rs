//! Unix filesystem access: every name resolves against an open directory, never a path.
//! rustix's `openat` family closes the check-then-open ancestor/symlink race. The local filesystem
//! must support hard links and directory fsync.
use super::root::resolve;
use rustix::fd::OwnedFd;
use rustix::fs::{AtFlags, Mode, OFlags, linkat, mkdirat, open, openat, unlinkat};
use rustix::io::Errno;
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read},
    path::{Component, Path},
};

/// Read only, reject links, avoid FIFO blocking, and keep the descriptor out of child processes.
const READ_REGULAR_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);
/// Write only, create exclusively without following links; CLOEXEC is observable only by a child
/// started while the descriptor is open.
const CREATE_NEW_FLAGS: OFlags = OFlags::WRONLY
    .union(OFlags::CREATE)
    .union(OFlags::EXCL)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
/// Give a created file owner-only read and write access.
const CREATE_NEW_MODE: Mode = Mode::RUSR.union(Mode::WUSR);
/// Open directories read-only, without following links or inheriting descriptors.
const OPEN_CHILD_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
/// Read without following links or blocking on FIFOs; CLOEXEC is observable only by a child
/// started while the descriptor is open.
const OPEN_NOFOLLOW_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::CLOEXEC)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK);

/// An open directory: names inside it resolve against the handle, never against a path.
#[derive(Debug)]
pub struct Directory(File);

impl Directory {
    /// Open the directory `below` names under the caller's `root`: the root resolves once, then
    /// the walk opens every component from `/` on without following links, creating missing
    /// components when asked.
    ///
    /// # Errors
    /// Returns an error for unsafe names, missing roots, or filesystem failures.
    pub fn open(root: &Path, below: &Path, create: bool) -> io::Result<Self> {
        let path = resolve(root, below)?;
        let mut directory = File::open("/")?;
        for component in path.components() {
            if let Component::Normal(name) = component {
                directory = File::from(open_child(&directory, name, create)?);
            }
        }
        Ok(Self(directory))
    }

    /// The bytes of a regular file in the directory, never read through a link.
    ///
    /// # Errors
    /// Returns an error if the name is unsafe, linked, non-regular, or unreadable.
    pub fn read_regular(&self, name: &str) -> io::Result<Vec<u8>> {
        let fd = openat(&self.0, name, READ_REGULAR_FLAGS, Mode::empty())?;
        let mut file = File::from(fd);
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("artifact is not a regular file"));
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Create a file the name must not already hold, link or not, writable by its owner alone.
    ///
    /// # Errors
    /// Returns an error if the name exists, is unsafe, or cannot be created.
    pub fn create_new(&self, name: &str) -> io::Result<File> {
        let fd = openat(&self.0, name, CREATE_NEW_FLAGS, CREATE_NEW_MODE)?;
        Ok(File::from(fd))
    }

    /// Link `from` as `to`, never replacing `to`, then flush the directory's entries.
    ///
    /// # Errors
    /// Returns an error if the link cannot be created or the directory cannot be flushed.
    pub fn link(&self, from: &str, to: &str) -> io::Result<()> {
        linkat(&self.0, from, &self.0, to, AtFlags::empty())?;
        self.0.sync_all()
    }

    /// Remove a name from the directory; a link is removed, never followed.
    ///
    /// # Errors
    /// Returns an error if the name cannot be removed.
    pub fn remove_file(&self, name: &str) -> io::Result<()> {
        Ok(unlinkat(&self.0, name, AtFlags::empty())?)
    }
}

/// Open one child directory without following a link, creating it first when asked and absent.
fn open_child(directory: &File, name: &OsStr, create: bool) -> io::Result<OwnedFd> {
    match openat(directory, name, OPEN_CHILD_FLAGS, Mode::empty()) {
        Ok(child) => Ok(child),
        Err(Errno::NOENT) if create => {
            match mkdirat(directory, name, Mode::RWXU) {
                Ok(()) => directory.sync_all()?,
                Err(Errno::EXIST) => {}
                Err(error) => return Err(error.into()),
            }
            Ok(openat(directory, name, OPEN_CHILD_FLAGS, Mode::empty())?)
        }
        Err(error) => Err(error.into()),
    }
}

/// Open a file for reading without following a link in its last component and without blocking
/// on a FIFO.
///
/// # Errors
/// Returns an error if the path is linked or cannot be opened.
pub fn open_nofollow(path: &Path) -> io::Result<File> {
    let fd = open(path, OPEN_NOFOLLOW_FLAGS, Mode::empty())?;
    Ok(File::from(fd))
}
