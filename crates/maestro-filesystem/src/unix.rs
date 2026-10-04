//! Unix filesystem access: every name resolves against an open directory, never a path.
//! rustix's `openat` family closes the check-then-open ancestor/symlink race. The local filesystem
//! must support hard links, directory fsync, and `renameat2` with `RENAME_NOREPLACE`; Linux drvfs
//! and NFS return `EINVAL` and fail closed without that support.
use super::{
    listing::{self, Entry, EntryKind},
    read::{read_limited, read_prefix},
    root::{leaf_name, resolve},
};
use rustix::fd::OwnedFd;
use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, linkat, mkdirat, open, openat,
    renameat_with, statat, unlinkat,
};
#[cfg(target_os = "linux")]
use rustix::fs::{StatxFlags, statx};
use rustix::{io::Errno, process::getuid};
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read},
    os::unix::ffi::OsStrExt as _,
    path::{Component, Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The next process-local suffix for a collision-free quarantine name.
pub(super) static NEXT_QUARANTINE: AtomicUsize = AtomicUsize::new(0);

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
pub struct Directory(pub(super) File, pub(super) PathBuf);

impl Directory {
    /// Open the directory `below` names under the caller's `root`: the root resolves once, then
    /// the walk opens every component from `/` on without following links, creating missing
    /// components when asked.
    ///
    /// # Errors
    /// Returns an error for unsafe names, missing roots, or filesystem failures.
    pub fn open(root: &Path, below: &Path, create: bool) -> io::Result<Self> {
        Self::open_resolved(&resolve(root, below)?, create)
    }

    /// Walk an already resolved absolute path without resolving links again.
    pub(crate) fn open_resolved(path: &Path, create: bool) -> io::Result<Self> {
        let mut directory = File::open("/")?;
        for component in path.components() {
            if let Component::Normal(name) = component {
                directory = File::from(open_child(&directory, name, create)?);
            }
        }
        Ok(Self(directory, path.to_path_buf()))
    }

    /// Hold a previously canonicalized directory without resolving its path again.
    ///
    /// # Errors
    /// Refuses relative paths, links and unreadable components.
    pub fn open_canonical(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(io::Error::other("directory must be canonical and absolute"));
        }
        let mut directory = File::open("/")?;
        for component in path.components() {
            if let Component::Normal(name) = component {
                directory = File::from(open_child(&directory, name, false)?);
            }
        }
        Ok(Self(directory, path.to_path_buf()))
    }

    /// Hold the parent reached from this directory's handle, never a new path walk.
    ///
    /// # Errors
    /// Returns an error for an inaccessible parent or root.
    pub fn parent(&self) -> io::Result<Self> {
        let path = self
            .1
            .parent()
            .ok_or_else(|| io::Error::other("no parent"))?;
        let fd = openat(&self.0, "..", OPEN_CHILD_FLAGS, Mode::empty())?;
        Ok(Self(File::from(fd), path.to_path_buf()))
    }

    /// Hold a single child without following links.
    ///
    /// # Errors
    /// Refuses absent, linked or inaccessible children.
    pub fn child(&self, name: &str) -> io::Result<Self> {
        let fd = open_child(&self.0, OsStr::new(name), false)?;
        Ok(Self(File::from(fd), self.1.join(name)))
    }

    /// List at most `limit` entries of this held handle, without following links.
    ///
    /// # Errors
    /// Refuses an extra entry or any listing/no-follow metadata failure.
    pub fn list_bounded(&self, limit: usize) -> io::Result<Vec<Entry>> {
        self.list_bounded_excluding(limit, None)
    }

    /// List under a structural VCS boundary, excluding one exact name before
    /// metadata access and counting. This is not a pattern or prefix filter.
    ///
    /// # Errors
    /// Refuses an extra entry or any listing/no-follow metadata failure.
    pub fn list_bounded_excluding(
        &self,
        limit: usize,
        excluded: Option<&OsStr>,
    ) -> io::Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for entry in Dir::read_from(&self.0)? {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            if excluded == Some(OsStr::from_bytes(name.to_bytes())) {
                continue;
            }
            let metadata = statat(&self.0, name, AtFlags::SYMLINK_NOFOLLOW)?;
            let kind = match FileType::from_raw_mode(metadata.st_mode) {
                FileType::RegularFile => EntryKind::File,
                FileType::Directory => EntryKind::Directory,
                FileType::Symlink => EntryKind::Link,
                _ => EntryKind::Other,
            };
            listing::push(
                &mut entries,
                Entry {
                    name: OsStr::from_bytes(name.to_bytes()).to_owned(),
                    kind,
                },
                limit,
            )?;
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    /// Whether this held directory is a filesystem or mount root.
    ///
    /// # Errors
    /// Returns an error when held metadata cannot be verified.
    pub fn is_mount_root(&self) -> io::Result<bool> {
        if self.1.parent().is_none() || drive_mount(&self.1) {
            return Ok(true);
        }
        let parent = self.parent()?;
        #[cfg(target_os = "linux")]
        {
            mount_root_with(&self.0, &parent.0, |file| {
                let metadata = statx(file, "", AtFlags::EMPTY_PATH, StatxFlags::MNT_ID)?;
                Ok((
                    StatxFlags::from_bits_retain(metadata.stx_mask),
                    metadata.stx_mnt_id,
                ))
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            use std::os::unix::fs::MetadataExt as _;
            Ok(self.0.metadata()?.dev() != parent.0.metadata()?.dev())
        }
    }

    /// Read only an owner-only-writable directory/file pair, checked on held handles.
    ///
    /// # Errors
    /// Refuses foreign owners, group/world write, unreadable files, swaps and limits.
    pub fn read_preferences(&self, name: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        self.verify_named()?;
        private_metadata(&self.0)?;
        let file = self.open_regular_path(name)?;
        private_metadata(&file)?;
        let bytes = read_limited(file, max_bytes)?;
        self.verify_named()?;
        Ok(bytes)
    }

    /// The canonical spelling that still names this held directory.
    ///
    /// # Errors
    /// Refuses resolution failures, links, changed names and identity mismatches.
    pub fn canonical_path(&self) -> io::Result<PathBuf> {
        self.verify_named()?;
        self.finish_canonical(self.1.canonicalize()?)
    }

    /// Test-only resolver seam for a link swap during canonicalization.
    #[cfg(test)]
    pub(crate) fn canonical_path_with(
        &self,
        resolve: impl FnOnce(&Path) -> io::Result<PathBuf>,
    ) -> io::Result<PathBuf> {
        self.verify_named()?;
        self.finish_canonical(resolve(&self.1)?)
    }

    /// Bind the returned spelling to the same held object, then recheck the original name.
    fn finish_canonical(&self, canonical: PathBuf) -> io::Result<PathBuf> {
        self.verify_path(&canonical)?;
        self.verify_named()?;
        Ok(canonical)
    }

    /// Refuse replacement of this directory or a link in its ancestor chain.
    ///
    /// # Errors
    /// Refuses an identity mismatch, links or unverifiable metadata.
    pub fn verify_named(&self) -> io::Result<()> {
        self.verify_path(&self.1)
    }

    /// The one identity comparison for both original and resolved spellings.
    fn verify_path(&self, path: &Path) -> io::Result<()> {
        use std::os::unix::fs::MetadataExt as _;
        let held = self.0.metadata()?;
        let named = Self::open_canonical(path)?.0.metadata()?;
        if held.dev() != named.dev() || held.ino() != named.ino() {
            return Err(io::Error::other("directory changed during discovery"));
        }
        Ok(())
    }

    /// The bytes of a regular file in the directory, never read through a link.
    ///
    /// # Errors
    /// Returns an error if the name is unsafe, linked, non-regular, or unreadable.
    pub fn read_regular(&self, name: &str) -> io::Result<Vec<u8>> {
        let mut file = self.open_regular_path(name)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Read at most `max_bytes + 1` bytes of a regular file through this held handle.
    /// Retains the sentinel byte for callers that diagnose oversize sources themselves.
    ///
    /// # Errors
    /// Refuses unsafe names, links, non-regular files and failed reads.
    pub fn read_regular_prefix(&self, name: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        read_prefix(self.open_regular_path(name)?, max_bytes)
    }

    /// The bytes of a regular file, never read through a link or beyond `max_bytes + 1`.
    ///
    /// # Errors
    /// Returns `io::ErrorKind::FileTooLarge` when the file exceeds `max_bytes`, or an error
    /// if the name is unsafe, linked, non-regular, or unreadable.
    pub fn read_regular_bounded(&self, name: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        read_limited(self.open_regular_path(name)?, max_bytes)
    }

    /// Open exactly one regular-file leaf through the held, still-named directory.
    ///
    /// # Errors
    /// Refuses traversal, separators/streams, replaced parents, links, non-regular files
    /// and failed opens. The returned handle, not the path, must be used for reading.
    pub fn open_regular(&self, name: &str) -> io::Result<File> {
        leaf_name(name)?;
        self.verify_named()?;
        let file = self.open_regular_path(name)?;
        self.verify_named()?;
        Ok(file)
    }

    /// Existing internal readers also use relative paths for quarantine operations.
    fn open_regular_path(&self, name: &str) -> io::Result<File> {
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
    /// A same-user process can replace the quarantine entry between the last check and unlinkat
    /// because POSIX has no unlink-if-same-file.
    ///
    /// # Errors
    /// Returns an error if the entry changes, is not regular, or cannot be safely restored/removed.
    pub fn remove_verified(
        &self,
        name: &str,
        expected: &[u8],
        expected_identity: Option<(u64, u64)>,
    ) -> io::Result<()> {
        self.remove_verified_checked(name, expected, expected_identity, || Ok(()))
    }

    /// Remove through quarantine, restoring the original name if the policy callback refuses.
    ///
    /// # Errors
    /// Refuses policy changes, mismatched bytes/identity and filesystem failures.
    pub fn remove_verified_checked(
        &self,
        name: &str,
        expected: &[u8],
        expected_identity: Option<(u64, u64)>,
        policy: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<()> {
        self.remove_verified_impl(name, expected, expected_identity, policy)
    }

    /// Inject scheduling without overriding the production policy callback.
    #[cfg(test)]
    pub(crate) fn remove_verified_with(
        &self,
        name: &str,
        expected: &[u8],
        expected_identity: Option<(u64, u64)>,
        after_quarantine: impl FnOnce(),
    ) -> io::Result<()> {
        self.remove_verified_impl(name, expected, expected_identity, || {
            after_quarantine();
            Ok(())
        })
    }

    /// One shared removal path for policy callbacks and byte/identity verification.
    fn remove_verified_impl(
        &self,
        name: &str,
        expected: &[u8],
        expected_identity: Option<(u64, u64)>,
        policy: impl FnOnce() -> io::Result<()>,
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
        self.remove_quarantined(
            name,
            || {},
            |quarantine| {
                policy()?;
                let fd = openat(&self.0, quarantine, READ_REGULAR_FLAGS, Mode::empty())?;
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
                if bytes != expected || !identity_matches {
                    return Err(io::Error::other(
                        "verified removal refused: file bytes changed",
                    ));
                }
                Ok(())
            },
        )
    }

    /// Share the non-replacing quarantine and restoration for byte and held-identity checks.
    pub(super) fn remove_quarantined(
        &self,
        name: &str,
        after_quarantine: impl FnOnce(),
        verify: impl FnOnce(&str) -> io::Result<()>,
    ) -> io::Result<()> {
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
        match verify(&quarantine) {
            Ok(()) => Ok(unlinkat(&self.0, quarantine.as_str(), AtFlags::empty())?),
            Err(error) => {
                if let Err(error) = self.link(&quarantine, name) {
                    return Err(io::Error::other(format!(
                        "verified removal refused; changed bytes retained at {quarantine}: {error}"
                    )));
                }
                self.remove_file(&quarantine)?;
                Err(error)
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

/// Windows drive mounts under WSL are never workspace roots, even on user-owned drvfs.
fn drive_mount(path: &Path) -> bool {
    path.parent() == Some(Path::new("/mnt"))
        && path.file_name().is_some_and(|name| {
            let name = name.as_encoded_bytes();
            name.len() == 1 && name.first().is_some_and(u8::is_ascii_alphabetic)
        })
}

/// Verify owner/write and owner-read bits using metadata from the open descriptor.
pub(super) fn private_metadata(file: &File) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = file.metadata()?;
    if metadata.uid() != getuid().as_raw()
        || metadata.mode() & 0o022 != 0
        || metadata.mode() & 0o400 == 0
    {
        return Err(io::Error::other(
            "foreign-owned, other-writable or unreadable preferences",
        ));
    }
    Ok(())
}

/// Bind mounts can share a device; Linux mount IDs must be verified on both held handles.
#[cfg(target_os = "linux")]
fn mount_root_with(
    directory: &File,
    parent: &File,
    mut probe: impl FnMut(&File) -> io::Result<(StatxFlags, u64)>,
) -> io::Result<bool> {
    let mut read = |file| {
        let (mask, id) = probe(file)
            .map_err(|error| io::Error::other(format!("mount ID unverifiable: {error}")))?;
        if !mask.contains(StatxFlags::MNT_ID) {
            return Err(io::Error::other(
                "mount ID unverifiable: STATX_MNT_ID unavailable",
            ));
        }
        Ok(id)
    };
    Ok(read(directory)? != read(parent)?)
}

#[cfg(all(test, target_os = "linux"))]
mod mount_tests {
    use crate::unix::mount_root_with;
    use rustix::fs::StatxFlags;
    use std::{fs::File, io};

    #[test]
    fn different_mount_ids_detect_mount_roots_even_on_one_device() {
        let file = File::open("/").unwrap();
        let mut next = 0;
        assert!(
            mount_root_with(&file, &file, |_| {
                next += 1;
                Ok((StatxFlags::MNT_ID, next))
            })
            .unwrap()
        );
        assert!(!mount_root_with(&file, &file, |_| Ok((StatxFlags::MNT_ID, 1))).unwrap());
    }

    #[test]
    fn unverifiable_mount_ids_refuse_either_held_handle() {
        let file = File::open("/").unwrap();
        for (unavailable_at, syscall_error) in [(0, true), (0, false), (1, true), (1, false)] {
            let mut responses = [Ok((StatxFlags::MNT_ID, 1)), Ok((StatxFlags::MNT_ID, 1))];
            responses[unavailable_at] = if syscall_error {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "statx unavailable",
                ))
            } else {
                Ok((StatxFlags::BASIC_STATS, 1))
            };
            let mut responses = responses.into_iter();
            let error = mount_root_with(&file, &file, |_| responses.next().unwrap()).unwrap_err();
            let reason = if syscall_error {
                "statx unavailable"
            } else {
                "STATX_MNT_ID unavailable"
            };
            assert_eq!(
                error.to_string(),
                format!("mount ID unverifiable: {reason}")
            );
        }
    }
}
