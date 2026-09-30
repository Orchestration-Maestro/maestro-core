//! Windows filesystem access: held directories and open flags that never follow a link.
//! Names resolve by path, but every directory on the way is held open without
//! `FILE_SHARE_DELETE`, so none can be renamed, deleted or replaced while the store works under
//! it, and every open carries `FILE_FLAG_OPEN_REPARSE_POINT`, so a symbolic link or junction is
//! opened itself and refused, never followed. The standard library exposes these flags safely;
//! the single-bit constants are Win32's documented values, and each combined value is checked
//! against its bits at compile time. The local filesystem must support hard links; directories
//! are not flushed, which Windows does only through a writable handle (ADR-0018).
use super::root::resolve;
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

    /// The bytes of a regular file in the directory, never read through a link.
    ///
    /// # Errors
    /// Returns an error if the name is unsafe, linked, non-regular, or unreadable.
    pub fn read_regular(&self, name: &str) -> io::Result<Vec<u8>> {
        let mut file = open_nofollow(&self.path.join(name))?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("artifact is not a regular file"));
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
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
