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

/// An open directory: names inside it resolve against the handle, never against a path.
#[derive(Debug)]
pub(crate) struct Directory(File);

impl Directory {
    /// Open the directory `below` names under the caller's `root`: the root resolves once, then
    /// the walk opens every component from `/` on without following links, creating missing
    /// components when asked.
    pub(crate) fn open(root: &Path, below: &Path, create: bool) -> io::Result<Self> {
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
    pub(crate) fn read_regular(&self, name: &str) -> io::Result<Vec<u8>> {
        let fd = openat(
            &self.0,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let mut file = File::from(fd);
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("artifact is not a regular file"));
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Create a file the name must not already hold, link or not, writable by its owner alone.
    pub(crate) fn create_new(&self, name: &str) -> io::Result<File> {
        let fd = openat(
            &self.0,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?;
        Ok(File::from(fd))
    }

    /// Link `from` as `to`, never replacing `to`, then flush the directory's entries.
    pub(crate) fn link(&self, from: &str, to: &str) -> io::Result<()> {
        linkat(&self.0, from, &self.0, to, AtFlags::empty())?;
        self.0.sync_all()
    }

    /// Remove a name from the directory; a link is removed, never followed.
    pub(crate) fn remove_file(&self, name: &str) -> io::Result<()> {
        Ok(unlinkat(&self.0, name, AtFlags::empty())?)
    }

    /// A second handle on the same directory, for a test that reads it from another thread.
    #[cfg(test)]
    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(self.0.try_clone()?))
    }
}

/// Open one child directory without following a link, creating it first when asked and absent.
fn open_child(directory: &File, name: &OsStr, create: bool) -> io::Result<OwnedFd> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    match openat(directory, name, flags, Mode::empty()) {
        Ok(child) => Ok(child),
        Err(Errno::NOENT) if create => {
            match mkdirat(directory, name, Mode::RWXU) {
                Ok(()) => directory.sync_all()?,
                Err(Errno::EXIST) => {}
                Err(error) => return Err(error.into()),
            }
            Ok(openat(directory, name, flags, Mode::empty())?)
        }
        Err(error) => Err(error.into()),
    }
}

/// Open a file for reading without following a link in its last component and without blocking
/// on a FIFO.
pub(crate) fn open_nofollow(path: &Path) -> io::Result<File> {
    let fd = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?;
    Ok(File::from(fd))
}
