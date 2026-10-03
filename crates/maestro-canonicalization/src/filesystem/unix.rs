//! Unix filesystem access: every name resolves against an open directory, never a path.
//! rustix's `openat` family closes the check-then-open ancestor/symlink race. The local filesystem
//! must support hard links and directory fsync.
use super::root::resolve;
use rustix::fd::OwnedFd;
use rustix::fs::{AtFlags, Mode, OFlags, linkat, mkdirat, open, openat, unlinkat};
use rustix::io::Errno;
use std::{
    ffi::OsStr,
    fs::{File, Permissions},
    io::{self, Read},
    os::unix::fs::{MetadataExt, PermissionsExt},
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
        Self::open_resolved(&resolve(root, below)?, create)
    }

    /// Walk an already resolved absolute path, without resolving links again.
    pub(crate) fn open_resolved(path: &Path, create: bool) -> io::Result<Self> {
        let mut directory = File::open("/")?;
        for component in path.components() {
            if let Component::Normal(name) = component {
                directory = File::from(open_child(&directory, name, create)?);
            }
        }
        Ok(Self(directory))
    }

    /// Refuse relocation by comparing the named root with its held identity.
    pub(crate) fn validate_owned(&self, path: &Path) -> io::Result<()> {
        let named = Self::open_resolved(path, false)?;
        let held = self.0.metadata()?;
        let current = named.0.metadata()?;
        if (held.dev(), held.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other("owned root was relocated or replaced"));
        }
        Ok(())
    }

    /// Locks and control creation require an owner-only lock domain.
    pub(crate) fn validate_private(&self) -> io::Result<()> {
        if self.0.metadata()?.mode() & 0o7777 != 0o700 {
            return Err(io::Error::other(
                "owned root must have mode 0700; run setup --yes",
            ));
        }
        Ok(())
    }

    /// Secure the held directory, never a subsequently reopened path.
    pub(crate) fn make_private(&self) -> io::Result<()> {
        self.0.set_permissions(Permissions::from_mode(0o700))
    }

    /// Open a permanent control file with no-follow and no truncation.
    pub(crate) fn open_control(&self, name: &str, create: bool) -> io::Result<File> {
        let mut flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        if create {
            flags |= OFlags::CREATE | OFlags::EXCL;
        }
        let file = File::from(openat(&self.0, name, flags, Mode::RUSR | Mode::WUSR)?);
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(io::Error::other(
                "control file must be regular with one link",
            ));
        }
        if metadata.mode() & 0o7777 != 0o600 || metadata.uid() != self.0.metadata()?.uid() {
            return Err(io::Error::other(
                "control file must have its root's owner and mode 0600",
            ));
        }
        if create {
            file.sync_all()?;
            self.0.sync_all()?;
        }
        Ok(file)
    }

    /// Bind a held control to its current, safely reopened name.
    pub(crate) fn validate_control(&self, name: &str, file: &File) -> io::Result<()> {
        let named = self.open_control(name, false)?;
        let held = file.metadata()?;
        let current = named.metadata()?;
        if (held.dev(), held.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other("permanent control file was replaced"));
        }
        Ok(())
    }

    /// Create exactly one new private directory beneath the held parent.
    pub(crate) fn reserve_child(&self, name: &str) -> io::Result<Self> {
        mkdirat(&self.0, name, Mode::RWXU)?;
        self.0.sync_all()?;
        Ok(Self(File::from(openat(
            &self.0,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?)))
    }

    /// Install a closed single-link file with anchored link/unlink and directory durability.
    /// The application's permanent guards exclude supported concurrent name changes.
    pub(crate) fn install_from(&self, staging: &Self, name: &str) -> io::Result<()> {
        let held = staging.open_receipt_file(name)?;
        held.sync_all()?;
        let named = staging.open_receipt_file(name)?;
        let before = held.metadata()?;
        let current = named.metadata()?;
        if (before.dev(), before.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other("publication source was replaced"));
        }
        linkat(&staging.0, name, &self.0, name, AtFlags::empty())?;
        self.0.sync_all()?;
        // A failure here preserves the installed name and any remaining staging link.
        // The lifecycle reports the partial install, never retries the closed writer.
        staging.remove_file(name)?;
        staging.0.sync_all()?;
        self.0.sync_all()
    }

    /// Open only a regular single-link receipt, anchored below this root.
    pub(crate) fn open_receipt_file(&self, name: &str) -> io::Result<File> {
        let file = File::from(openat(
            &self.0,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.uid() != self.0.metadata()?.uid()
        {
            return Err(io::Error::other(
                "receipt file must be regular, single-link and root-owned",
            ));
        }
        Ok(file)
    }

    /// Recheck the held identity immediately before one anchored unlink, then sync.
    pub(crate) fn remove_receipt_file(
        &self,
        name: &str,
        expected: Option<&File>,
    ) -> io::Result<bool> {
        let named = match self.open_receipt_file(name) {
            Ok(named) => named,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.0.sync_all()?;
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        let held = expected
            .ok_or_else(|| io::Error::other("receipt file appeared after lookup"))?
            .metadata()?;
        let current = named.metadata()?;
        if (held.dev(), held.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other("receipt file identity was replaced"));
        }
        unlinkat(&self.0, name, AtFlags::empty())?;
        self.0.sync_all()?;
        Ok(true)
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
