//! Windows filesystem access: held directories and open flags that never follow a link.
//! Names resolve by path, but every directory on the way is held open without
//! `FILE_SHARE_DELETE`, so ordinary held parents cannot be renamed or replaced during effects.
//! A newly created child temporarily permits deletion for rollback, then is hardened.
//! Regular-file reads retain the verified object even if another process renames its name.
//! Every open carries
//! `FILE_FLAG_OPEN_REPARSE_POINT`, so a symbolic link or junction is
//! opened itself and refused, never followed. The standard library exposes these flags safely;
//! the single-bit constants are Win32's documented values, and fixed combined constants are checked
//! against its bits at compile time. The local filesystem must support hard links; directories
//! are not flushed, which Windows does only through a writable handle (ADR-0018).
use super::{
    listing::{self, Entry},
    read::{read_limited, read_prefix},
    root::{leaf_name, resolve},
    windows_flags::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ_WRITE,
        OPEN_REPARSE_DIRECTORY_FLAGS, file_share_all,
    },
    windows_security::{private_metadata, remove_created_directory, same_file},
};
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Read},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The next process-local suffix for a collision-free quarantine name.
static NEXT_QUARANTINE: AtomicUsize = AtomicUsize::new(0);

/// `FILE_ATTRIBUTE_DIRECTORY`: the handle names a directory.
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
/// `FILE_ATTRIBUTE_REPARSE_POINT`: the handle names a reparse point, a link or junction among them.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

/// A directory reached by a path whose every directory, from its anchor on, is held open.
#[derive(Debug)]
pub struct Directory {
    /// The path walked; names inside the directory are opened below it.
    path: PathBuf,
    /// The anchor and each component, which no one can rename or delete while they stay open.
    held: Vec<File>,
}

impl Directory {
    /// Windows std rename uses `MoveFileExW` with replace-existing, under held parents.
    pub(super) fn rename_replacement(&self, from: &str, to: &str) -> io::Result<()> {
        fs::rename(self.path.join(from), self.path.join(to))
    }

    /// Reopen the empty staged sibling for security writes and verify its created identity.
    pub(super) fn retain_replacement_security(
        &self,
        name: &str,
        staged: &File,
        original: &File,
    ) -> io::Result<()> {
        use windows_sys::Win32::{
            Foundation::GENERIC_READ,
            Storage::FileSystem::{WRITE_DAC, WRITE_OWNER},
        };
        let security = OpenOptions::new()
            .access_mode(GENERIC_READ | WRITE_DAC | WRITE_OWNER)
            .share_mode(file_share_all())
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(self.path.join(name))?;
        if !same_file(staged, &security)? {
            return Err(io::Error::other("staged replacement identity changed"));
        }
        super::windows_security::retain_replacement_security(original, &security)
    }

    /// Open the directory `below` names under the caller's `root`: the root resolves once, to a
    /// verbatim path such as `\\?\C:\data`, then the walk holds every component from its drive or
    /// share on, never following a link, and creates missing components when asked.
    ///
    /// # Errors
    /// Returns an error for unsafe names, missing roots, or filesystem failures.
    pub fn open(root: &Path, below: &Path, create: bool) -> io::Result<Self> {
        let path = resolve(root, below)?;
        // The prefix and root of a resolved path, such as `\\?\C:\` or `\\?\UNC\server\share\`.
        let start: PathBuf = path
            .components()
            .take_while(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
            .collect();
        // The anchor, like `/` on Unix, is trusted as it resolves.
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

    /// Hold a canonical directory without resolving links a second time.
    ///
    /// # Errors
    /// Refuses relative paths, reparse points and unreadable components.
    pub fn open_canonical(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(io::Error::other("directory must be canonical and absolute"));
        }
        let start: PathBuf = path
            .components()
            .take_while(|part| matches!(part, Component::Prefix(_) | Component::RootDir))
            .collect();
        let mut directory = Self {
            held: vec![hold(&start, FILE_FLAG_BACKUP_SEMANTICS)?],
            path: start,
        };
        for part in path.components() {
            if let Component::Normal(name) = part {
                directory = open_child(directory, name, false)?;
            }
        }
        Ok(directory)
    }

    /// Hold the already-held parent; no ancestor is reopened by path.
    ///
    /// # Errors
    /// Returns an error for root or a failed handle duplication.
    pub fn parent(&self) -> io::Result<Self> {
        let path = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("no parent"))?;
        Ok(Self {
            path: path.to_path_buf(),
            held: self
                .held
                .iter()
                .take(self.held.len().saturating_sub(1))
                .map(File::try_clone)
                .collect::<io::Result<_>>()?,
        })
    }

    /// Hold one child without following a reparse point.
    ///
    /// # Errors
    /// Refuses missing, linked or unreadable children.
    pub fn child(&self, name: &str) -> io::Result<Self> {
        let clone = Self {
            path: self.path.clone(),
            held: self
                .held
                .iter()
                .map(File::try_clone)
                .collect::<io::Result<_>>()?,
        };
        open_child(clone, OsStr::new(name), false)
    }

    /// Exclusively create and hold a child; only the new child allows delete sharing for rollback.
    ///
    /// # Errors
    /// Refuses existing names, reparse points and failed creation or opens.
    pub fn create_child(&self, name: &str) -> io::Result<Self> {
        leaf_name(name)?;
        let path = self.path.join(name);
        fs::create_dir(&path)?;
        let child = OpenOptions::new()
            .read(true)
            .share_mode(file_share_all())
            .custom_flags(OPEN_REPARSE_DIRECTORY_FLAGS)
            .open(&path)?;
        refuse_reparse_point(&child)?;
        let mut held = self
            .held
            .iter()
            .map(File::try_clone)
            .collect::<io::Result<Vec<_>>>()?;
        held.push(child);
        Ok(Self { path, held })
    }

    /// Replace temporary delete sharing with the normal non-renamable ancestor lease.
    ///
    /// # Errors
    /// Refuses a full-identity mismatch, reparse points and failed opens.
    pub fn harden_created_child(&self, name: &str, created: &Self) -> io::Result<Self> {
        let child = self.child(name)?;
        let held = created
            .held
            .last()
            .ok_or_else(|| io::Error::other("no created handle"))?;
        self.verify_created(name, held)?;
        Ok(child)
    }

    /// Delete the still-empty created directory by a full-identity checked native handle.
    ///
    /// # Errors
    /// Refuses replacements, non-empty directories and failed native disposition.
    pub fn remove_created_child(&self, name: &str, created: &Self) -> io::Result<()> {
        leaf_name(name)?;
        let held = created
            .held
            .last()
            .ok_or_else(|| io::Error::other("no created handle"))?;
        remove_created_directory(&self.path.join(name), held)
    }

    /// List at most `limit` entries below held, non-renamable ancestors.
    /// Every entry is opened with the Win32 no-follow flag before classification.
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
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            if excluded == Some(entry.file_name().as_os_str()) {
                continue;
            }
            let file = hold(&entry.path(), OPEN_REPARSE_DIRECTORY_FLAGS)?;
            let metadata = file.metadata()?;
            let kind = listing::windows_kind(
                metadata.file_attributes(),
                metadata.is_dir(),
                metadata.is_file(),
            );
            listing::push(
                &mut entries,
                Entry {
                    name: entry.file_name(),
                    kind,
                },
                limit,
            )?;
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    /// Whether this directory is the drive/share anchor.
    ///
    /// # Errors
    /// This host's held ancestry makes root detection infallible.
    pub fn is_mount_root(&self) -> io::Result<bool> {
        Ok(self.held.len() == 1)
    }

    /// Bounded preferences read with owner SID and DACL checks on both held handles.
    ///
    /// # Errors
    /// Refuses unsafe/unverifiable ACLs, foreign ownership, reparse points and limits.
    pub fn read_preferences(&self, name: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        let directory = self
            .held
            .last()
            .ok_or_else(|| io::Error::other("no held directory"))?;
        private_metadata(directory)?;
        let file = hold(&self.path.join(name), FILE_FLAG_OPEN_REPARSE_POINT)?;
        refuse_reparse_point(&file)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("preferences are not a regular file"));
        }
        private_metadata(&file)?;
        read_limited(file, max_bytes)
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

    /// Recheck the named directory chain. Held handles exclude deletion and renaming
    /// on Windows, so successful no-follow opens establish the same identity.
    ///
    /// # Errors
    /// Refuses missing paths, reparse points and failed opens.
    pub fn verify_named(&self) -> io::Result<()> {
        self.verify_path(&self.path)
    }

    /// The canonical spelling that provably names this held directory.
    ///
    /// # Errors
    /// Refuses resolution failures, reparse points, changed names and identity mismatches.
    pub fn canonical_path(&self) -> io::Result<PathBuf> {
        self.verify_named()?;
        let canonical = self.path.canonicalize()?;
        self.verify_path(&canonical)?;
        self.verify_named()?;
        Ok(canonical)
    }

    /// One full-identity comparison for both original and resolved directory spellings.
    fn verify_path(&self, path: &Path) -> io::Result<()> {
        let held = self
            .held
            .last()
            .ok_or_else(|| io::Error::other("no held directory"))?;
        let named = Self::open_canonical(path)?;
        let named = named
            .held
            .last()
            .ok_or_else(|| io::Error::other("no named directory"))?;
        if !same_file(held, named)? {
            return Err(io::Error::other("directory changed during discovery"));
        }
        Ok(())
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
        let file = open_nofollow(&self.path.join(name))?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("artifact is not a regular file"));
        }
        Ok(file)
    }

    /// Compare a no-follow named object to its retained full volume and 128-bit file identity.
    ///
    /// # Errors
    /// Refuses reparse points, replacements and failed opens or identity queries.
    pub fn verify_created(&self, name: &str, created: &File) -> io::Result<()> {
        leaf_name(name)?;
        let named = hold(&self.path.join(name), OPEN_REPARSE_DIRECTORY_FLAGS)?;
        refuse_reparse_point(&named)?;
        if !same_file(created, &named)? {
            return Err(io::Error::other(
                "created object changed; replacement not deleted",
            ));
        }
        Ok(())
    }

    /// Create a file the name must not already hold, link or not.
    ///
    /// # Errors
    /// Returns an error if the name exists, is unsafe, or cannot be created.
    pub fn create_new(&self, name: &str) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(self.path.join(name))
    }

    /// Flush this directory's entries. Windows cannot flush a directory through std.
    ///
    /// # Errors
    /// This operation currently cannot report a flush error.
    pub fn sync(&self) -> io::Result<()> {
        Ok(())
    }

    /// Link `from` as `to`, never replacing `to`. The directory is not flushed.
    ///
    /// # Errors
    /// Returns an error if the link cannot be created.
    pub fn link(&self, from: &str, to: &str) -> io::Result<()> {
        fs::hard_link(self.path.join(from), self.path.join(to))
    }

    /// Remove `name` only if its bytes still match `expected`, without following links.
    ///
    /// The entry is first renamed into an exclusively created quarantine directory inside this held
    /// directory. Verification opens it without following reparse points. A mismatch is restored
    /// with a non-replacing hard link; changed bytes are never removed. The predictable quarantine
    /// directory is visible, so another process creating a file inside it during the rename window
    /// can prevent safe completion.
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

    /// Restore the quarantined name on policy refusal before unlinking its bytes.
    ///
    /// # Errors
    /// Refuses changed policy, bytes, unsupported identity and filesystem failures.
    pub fn remove_verified_checked(
        &self,
        name: &str,
        expected: &[u8],
        expected_identity: Option<(u64, u64)>,
        policy: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<()> {
        self.remove_verified_impl(name, (expected, expected_identity), None, policy)
    }

    /// Test scheduling seam through the same quarantine implementation.
    #[cfg(test)]
    pub(crate) fn remove_verified_with(
        &self,
        name: &str,
        expected: &[u8],
        expected_identity: Option<(u64, u64)>,
        after_quarantine: impl FnOnce(),
    ) -> io::Result<()> {
        self.remove_verified_impl(name, (expected, expected_identity), None, || {
            after_quarantine();
            Ok(())
        })
    }

    /// Shared policy-and-byte verification before unlink or non-replacing restore.
    fn remove_verified_impl(
        &self,
        name: &str,
        (expected, expected_identity): (&[u8], Option<(u64, u64)>),
        created: Option<&File>,
        policy: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<()> {
        if expected_identity.is_some() {
            return Err(io::Error::new(
                ErrorKind::Unsupported,
                "stable file identity is unavailable on Windows",
            ));
        }
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name == "."
            || name == ".."
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "name is not one file",
            ));
        }
        let original = open_nofollow(&self.path.join(name))?;
        if !original.metadata()?.is_file() {
            return Err(io::Error::other("artifact is not a regular file"));
        }
        drop(original);
        let quarantine = loop {
            let counter = NEXT_QUARANTINE.fetch_add(1, Ordering::Relaxed);
            let candidate = format!(".{name}.maestro-quarantine-{}-{counter}", process::id());
            match fs::create_dir(self.path.join(&candidate)) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        };
        let quarantine_file = format!("{quarantine}/file");
        if let Err(error) = fs::rename(self.path.join(name), self.path.join(&quarantine_file)) {
            fs::remove_dir(self.path.join(&quarantine))?;
            return Err(error);
        }
        let verification = policy().and_then(|()| {
            let mut file = self.open_regular_path(&quarantine_file)?;
            if let Some(created) = created
                && !same_file(created, &file)?
            {
                return Err(io::Error::other(
                    "created file changed; replacement not deleted",
                ));
            }
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        });
        match verification {
            Ok(bytes) if bytes == expected => {
                self.remove_file(&quarantine_file)?;
                fs::remove_dir(self.path.join(&quarantine))
            }
            refused => {
                let error = refused.map_or_else(
                    |error| error,
                    |_| io::Error::other("verified removal refused: file bytes changed"),
                );
                if let Err(error) = self.link(&quarantine_file, name) {
                    return Err(io::Error::other(format!(
                        "verified removal refused; changed bytes retained at {quarantine}: {error}"
                    )));
                }
                self.remove_file(&quarantine_file)?;
                fs::remove_dir(self.path.join(&quarantine))?;
                Err(error)
            }
        }
    }

    /// Roll back only the bytes and full identity of a file created by this operation.
    ///
    /// # Errors
    /// Refuses replaced names, edited bytes and cleanup failures.
    pub fn remove_created_bytes(&self, name: &str, created: &File, bytes: &[u8]) -> io::Result<()> {
        self.remove_verified_impl(name, (bytes, None), Some(created), || Ok(()))
    }

    /// Remove a name from the directory; a link is removed, never followed.
    ///
    /// # Errors
    /// Returns an error if the name cannot be removed.
    pub fn remove_file(&self, name: &str) -> io::Result<()> {
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
    let held = hold(path, OPEN_REPARSE_DIRECTORY_FLAGS)?;
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
        .share_mode(FILE_SHARE_READ_WRITE)
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
///
/// # Errors
/// Returns an error if the path is linked or cannot be opened.
pub fn open_nofollow(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(OPEN_REPARSE_DIRECTORY_FLAGS)
        .open(path)?;
    refuse_reparse_point(&file)?;
    Ok(file)
}
