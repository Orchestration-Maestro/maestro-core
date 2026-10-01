//! Windows filesystem access: held directories and open flags that never follow a link.
//! Names resolve by path, but every directory on the way is held open without
//! `FILE_SHARE_DELETE`, so none can be renamed, deleted or replaced while the store works under
//! it, and every open carries `FILE_FLAG_OPEN_REPARSE_POINT`, so a symbolic link or junction is
//! opened itself and refused, never followed. The standard library exposes these flags safely;
//! the single-bit constants are Win32's documented values, and each combined value is checked
//! against its bits at compile time. The local filesystem must support hard links; directories
//! are not flushed, which Windows does only through a writable handle (ADR-0018).
use super::{
    listing::{self, Entry},
    read::{read_limited, read_prefix},
    root::{leaf_name, resolve},
    windows_security::{private_metadata, same_file},
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

/// `FILE_SHARE_READ`: others may read the file while the handle is open.
const FILE_SHARE_READ: u32 = 0x0000_0001;
/// `FILE_SHARE_WRITE`: others may write the file while the handle is open.
const FILE_SHARE_WRITE: u32 = 0x0000_0002;
/// `FILE_SHARE_READ | FILE_SHARE_WRITE`: never deletion or renaming while held.
const FILE_SHARE_READ_WRITE: u32 = 0x0000_0003;
/// `FILE_FLAG_BACKUP_SEMANTICS`: the open may name a directory.
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
/// `FILE_FLAG_OPEN_REPARSE_POINT`: a reparse point is opened itself, never followed.
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
/// `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT`: a directory may open, and a
/// reparse point opens itself.
const OPEN_REPARSE_DIRECTORY_FLAGS: u32 = 0x0220_0000;
// Each precombined value is exactly its named Win32 bits.
const _: () = assert!(FILE_SHARE_READ_WRITE == FILE_SHARE_READ | FILE_SHARE_WRITE);
const _: () = assert!(
    OPEN_REPARSE_DIRECTORY_FLAGS == FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
);
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
        after_quarantine();
        match self.read_regular(&quarantine_file) {
            Ok(bytes) if bytes == expected => {
                self.remove_file(&quarantine_file)?;
                fs::remove_dir(self.path.join(&quarantine))
            }
            Ok(_) | Err(_) => {
                if let Err(error) = self.link(&quarantine_file, name) {
                    return Err(io::Error::other(format!(
                        "verified removal refused; changed bytes retained at {quarantine}: {error}"
                    )));
                }
                self.remove_file(&quarantine_file)?;
                fs::remove_dir(self.path.join(&quarantine))?;
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
