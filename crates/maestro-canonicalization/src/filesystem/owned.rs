//! Owned filesystem roots and permanent control files, independent of any engine.
use super::root::resolve;
#[cfg(unix)]
use super::unix::Directory;
#[cfg(windows)]
use super::windows::Directory;
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// A safely held, application-owned directory. Its parent is trusted configuration
/// and resolves once; the owned leaf and every later operation refuse links.
/// Windows ACL privacy relies on inheritance from the user-private data directory;
/// it is not separately verified (no approved ACL API).
#[derive(Clone, Debug)]
pub struct OwnedRoot {
    /// The resolved location whose identity must still match the held root.
    path: PathBuf,
    /// Platform capability retaining the root (and Windows ancestors).
    directory: Arc<Directory>,
}

/// The permanent control-file domain under an owned directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlFile {
    /// Readers and writers share this file; removal takes it exclusively.
    Access,
    /// Serializes producers independently of readers.
    Writer,
}

impl ControlFile {
    /// The fixed basename; callers cannot select a different lock domain.
    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Access => ".access.guard",
            Self::Writer => ".writer.guard",
        }
    }
}

/// A safely opened permanent control file. Its retained root outlives its lock;
/// dropping the handle releases the OS lock. Direct engine-file access outside
/// this control-file protocol is unsupported.
#[derive(Debug)]
pub struct ControlHandle {
    /// Retains the safely opened file and any acquired lock.
    file: File,
    /// The lock domain must remain at its original location.
    root: OwnedRoot,
    /// The permanent basename bound to this handle.
    control: ControlFile,
    /// Serialize mode changes with removal so a shared downgrade cannot race unlink.
    mode: Mutex<Option<LockMode>>,
}

/// The requested filesystem lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockMode {
    /// Coexists with other shared holders.
    Shared,
    /// Refuses all competing holders.
    Exclusive,
}

/// An injectable platform file-lock primitive. Unsupported must remain an error.
pub trait FileLock {
    /// Acquire the requested lock, waiting only when requested.
    ///
    /// # Errors
    /// Returns contention, unsupported locking or other filesystem errors unchanged.
    fn acquire(&self, file: &File, mode: LockMode, wait: bool) -> io::Result<()>;
}

/// The standard library's OS file-lock adapter. There is no fallback lock domain.
#[derive(Debug)]
pub struct SystemFileLock;

impl FileLock for SystemFileLock {
    fn acquire(&self, file: &File, mode: LockMode, wait: bool) -> io::Result<()> {
        match (mode, wait) {
            (LockMode::Shared, true) => file.lock_shared(),
            (LockMode::Exclusive, true) => file.lock(),
            (LockMode::Shared, false) => file.try_lock_shared().map_err(io::Error::from),
            (LockMode::Exclusive, false) => file.try_lock().map_err(io::Error::from),
        }
    }
}

impl OwnedRoot {
    /// Open an owned root, optionally creating missing directories with mode
    /// `0700` on Unix or the parent's private inherited ACL on Windows.
    ///
    /// # Errors
    /// Refuses unsafe filesystem objects, parent traversal and filesystem failures.
    pub fn open(path: &Path, create: bool) -> io::Result<Self> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("owned root needs a parent"))?;
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::other("owned root needs a name"))?;
        let path = resolve(parent, Path::new(name))?;
        let directory = Arc::new(Directory::open_resolved(&path, create)?);
        let root = Self { path, directory };
        root.validate()?;
        Ok(root)
    }

    /// Give the held root private Unix permissions; Windows retains the inherited ACL.
    ///
    /// # Errors
    /// Refuses relocation or failure to change the held directory's permissions.
    pub fn make_private(&self) -> io::Result<()> {
        self.validate()?;
        #[cfg(unix)]
        self.directory.make_private()?;
        Ok(())
    }

    /// Create only a missing control file; return whether it was created.
    /// Existing controls are validated, never replaced or truncated.
    ///
    /// # Errors
    /// Refuses links, nonregular files, hard-link aliases, unsafe permissions,
    /// relocated roots and filesystem failures.
    pub fn ensure_control(&self, control: ControlFile) -> io::Result<bool> {
        self.validate()?;
        #[cfg(unix)]
        self.directory.validate_private()?;
        let created = match self.directory.open_control(control.file_name(), true) {
            Ok(_) => true,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                self.open_control(control)?;
                false
            }
            Err(error) => return Err(error),
        };
        self.validate()?;
        Ok(created)
    }

    /// Open an existing control file without creating or truncating it.
    ///
    /// # Errors
    /// Refuses missing/unsafe files and relocated roots. Missing files are repaired
    /// only by the application's explicit setup operation, not by readers/writers.
    pub fn open_control(&self, control: ControlFile) -> io::Result<ControlHandle> {
        self.validate()?;
        let file = self.directory.open_control(control.file_name(), false)?;
        self.validate()?;
        Ok(ControlHandle {
            file,
            root: self.clone(),
            control,
            mode: Mutex::new(None),
        })
    }

    /// Confirm that the owned name still denotes the held directory.
    fn validate(&self) -> io::Result<()> {
        self.directory.validate_owned(&self.path)
    }
}

impl ControlHandle {
    /// Acquire through the injected adapter; the file remains held until this handle drops.
    ///
    /// # Errors
    /// Refuses unsupported locks, contention and relocated roots without fallback.
    pub fn lock_with(&self, adapter: &dyn FileLock, mode: LockMode, wait: bool) -> io::Result<()> {
        let mut held = self
            .mode
            .lock()
            .map_err(|_| io::Error::other("control mode lock poisoned"))?;
        self.validate()?;
        adapter.acquire(&self.file, mode, wait)?;
        *held = Some(mode);
        self.validate()
    }

    /// Refuse an opened control whose name now denotes a different file.
    fn validate(&self) -> io::Result<()> {
        self.root.validate()?;
        #[cfg(unix)]
        self.root.directory.validate_private()?;
        self.root
            .directory
            .validate_control(self.control.file_name(), &self.file)
    }
}

/// A held, no-follow regular receipt file, bound to its owned root and name.
#[derive(Debug)]
pub struct ReceiptFile {
    /// The safely opened file.
    file: File,
    /// The root capability that authorized this name.
    root: OwnedRoot,
    /// The immutable receipt basename.
    name: String,
}

impl OwnedRoot {
    /// Open a canonical receipt file without reading its content; absence is not an error.
    ///
    /// # Errors
    /// Refuses noncanonical names, unsafe objects, aliases and relocated roots.
    pub fn receipt_file(&self, name: &str) -> io::Result<Option<ReceiptFile>> {
        validate_receipt_name(name)?;
        self.validate()?;
        let file = match self.directory.open_receipt_file(name) {
            Ok(file) => Some(ReceiptFile {
                file,
                root: self.clone(),
                name: name.to_owned(),
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        self.validate()?;
        Ok(file)
    }
}

impl ControlHandle {
    /// Remove exactly the validated receipt file under an exclusive access guard.
    /// Return false for idempotent absence; sync the held directory in either case.
    ///
    /// # Errors
    /// Refuses unsafe identity changes, missing exclusive access, unsupported durability
    /// and filesystem errors. No path-only or recursive fallback exists.
    pub fn remove_receipt_file(
        &self,
        name: &str,
        expected: Option<&ReceiptFile>,
    ) -> io::Result<bool> {
        validate_receipt_name(name)?;
        self.validate()?;
        let held = self
            .mode
            .lock()
            .map_err(|_| io::Error::other("control mode lock poisoned"))?;
        if self.control != ControlFile::Access || *held != Some(LockMode::Exclusive) {
            return Err(io::Error::other(
                "receipt removal requires exclusive access guard",
            ));
        }
        if let Some(file) = expected
            && (file.name != name || !Arc::ptr_eq(&self.root.directory, &file.root.directory))
        {
            return Err(io::Error::other(
                "receipt file belongs to another root or name",
            ));
        }
        self.root
            .directory
            .remove_receipt_file(name, expected.map(|file| &file.file))
    }
}

/// Whether a graph receipt basename is canonical, fixed-width and companion-safe.
#[must_use]
pub fn is_receipt_basename(name: &str) -> bool {
    let Some(digest) = name
        .strip_prefix('g')
        .and_then(|name| name.strip_suffix(".lbdb"))
    else {
        return false;
    };
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Refuse names that cannot denote a canonical graph receipt.
fn validate_receipt_name(name: &str) -> io::Result<()> {
    if !is_receipt_basename(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "noncanonical graph receipt basename",
        ));
    }
    Ok(())
}
