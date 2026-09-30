//! Unix filesystem access: every name resolves against an open directory, never a path.
//! rustix's `openat` family closes the check-then-open ancestor/symlink race. The local filesystem
//! must support hard links, directory fsync, and `renameat2` with `RENAME_NOREPLACE`; Linux drvfs
//! and NFS return `EINVAL` and fail closed without that support.
use super::{read::read_limited, root::resolve};
use rustix::fd::OwnedFd;
use rustix::fs::{
    AtFlags, Mode, OFlags, RenameFlags, linkat, mkdirat, open, openat, renameat_with, unlinkat,
};
use rustix::io::Errno;
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read},
    path::{Component, Path},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The next process-local suffix for a collision-free quarantine name.
static NEXT_QUARANTINE: AtomicUsize = AtomicUsize::new(0);

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
        let mut file = self.open_regular(name)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// The bytes of a regular file, never read through a link or beyond `max_bytes + 1`.
    ///
    /// # Errors
    /// Returns `io::ErrorKind::FileTooLarge` when the file exceeds `max_bytes`, or an error
    /// if the name is unsafe, linked, non-regular, or unreadable.
    pub fn read_regular_bounded(&self, name: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        read_limited(self.open_regular(name)?, max_bytes)
    }

    /// Open a regular file through the held directory without following a link.
    fn open_regular(&self, name: &str) -> io::Result<File> {
        let fd = openat(&self.0, name, READ_REGULAR_FLAGS, Mode::empty())?;
        let file = File::from(fd);
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("artifact is not a regular file"));
        }
        Ok(file)
    }

    /// Create a file the name must not already hold, link or not, writable by its owner alone.
    ///
    /// # Errors
    /// Returns an error if the name exists, is unsafe, or cannot be created.
    pub fn create_new(&self, name: &str) -> io::Result<File> {
        let fd = openat(&self.0, name, CREATE_NEW_FLAGS, CREATE_NEW_MODE)?;
        Ok(File::from(fd))
    }

    /// Flush this directory's entries to stable storage.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be flushed.
    pub fn sync(&self) -> io::Result<()> {
        self.0.sync_all()
    }

    /// Link `from` as `to`, never replacing `to`, then flush the directory's entries.
    ///
    /// # Errors
    /// Returns an error if the link cannot be created or the directory cannot be flushed.
    pub fn link(&self, from: &str, to: &str) -> io::Result<()> {
        linkat(&self.0, from, &self.0, to, AtFlags::empty())?;
        self.sync()
    }

    /// Remove `name` only if its bytes still match `expected`, without following links.
    ///
    /// The entry is first atomically renamed to a predictable, visible quarantine name relative to
    /// this held directory. A mismatch is restored without replacing a concurrently-created name.
    /// If restoration meets a reappeared name, or a crash occurs between rename and unlink, the old
    /// bytes remain in the hidden quarantine; a resumed catalog removal then has no ownership
    /// record to proceed from. Verification precedes unlink-by-name, so a writer holding an open
    /// descriptor can still change bytes after verification.
    ///
    /// # Errors
    /// Returns an error if the entry changes, is not regular, or cannot be safely restored/removed.
    pub fn remove_verified(
        &self,
        name: &str,
        expected: &[u8],
        expected_identity: Option<(u64, u64)>,
    ) -> io::Result<()> {
        self.remove_verified_with(name, expected, expected_identity, || {})
    }

    /// Remove a verified file, invoking `after_quarantine` immediately after its rename.
    pub(crate) fn remove_verified_with(
        &self,
        name: &str,
        expected: &[u8],
        expected_identity: Option<(u64, u64)>,
        after_quarantine: impl FnOnce(),
    ) -> io::Result<()> {
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name == "."
            || name == ".."
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "name is not one file",
            ));
        }
        let original = openat(&self.0, name, OPEN_NOFOLLOW_FLAGS, Mode::empty())?;
        if !File::from(original).metadata()?.is_file() {
            return Err(io::Error::other("artifact is not a regular file"));
        }
        let quarantine = loop {
            let counter = NEXT_QUARANTINE.fetch_add(1, Ordering::Relaxed);
            let candidate = format!(".{name}.maestro-quarantine-{}-{counter}", process::id());
            match renameat_with(&self.0, name, &self.0, &candidate, RenameFlags::NOREPLACE) {
                Ok(()) => break candidate,
                Err(Errno::EXIST) => {}
                Err(error) => return Err(error.into()),
            }
        };
        after_quarantine();
        let opened = (|| {
            let fd = openat(
                &self.0,
                quarantine.as_str(),
                READ_REGULAR_FLAGS,
                Mode::empty(),
            )?;
            let mut file = File::from(fd);
            let metadata = file.metadata()?;
            if !metadata.is_file() {
                return Err(io::Error::other("artifact is not a regular file"));
            }
            let identity_matches = expected_identity.is_none_or(|(device, inode)| {
                use std::os::unix::fs::MetadataExt;
                metadata.dev() == device && metadata.ino() == inode
            });
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok((bytes, identity_matches))
        })();
        match opened {
            Ok((bytes, true)) if bytes == expected => {
                Ok(unlinkat(&self.0, quarantine.as_str(), AtFlags::empty())?)
            }
            Ok(_) | Err(_) => {
                if let Err(error) = self.link(&quarantine, name) {
                    return Err(io::Error::other(format!(
                        "verified removal refused; changed bytes retained at {quarantine}: {error}"
                    )));
                }
                self.remove_file(&quarantine)?;
                Err(io::Error::other(
                    "verified removal refused: file bytes changed",
                ))
            }
        }
    }

    /// Remove a name from the directory; a link is removed, never followed.
    ///
    /// # Errors
    /// Returns an error if the name cannot be removed.
    pub fn remove_file(&self, name: &str) -> io::Result<()> {
        unlinkat(&self.0, name, AtFlags::empty())?;
        self.0.sync_all()
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
