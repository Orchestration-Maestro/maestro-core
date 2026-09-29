//! One edit of a preferences file, from reading it to keeping or undoing
//! the change, under a lock every Maestro process takes for that file.
//!
//! [`FileEdit::begin`] opens the file's directory without following a
//! link, waits for the lock and reads the file. [`FileEdit::publish`]
//! checks that no other program changed the file since, keeps a recovery
//! copy of it beside it, writes the new text to a private new file with
//! the old file's permissions, and renames it over the file.
//! [`FileEdit::keep`] removes the recovery copy; [`FileEdit::undo`] puts
//! the old file back, confirms it, and names the recovery copy when it
//! cannot. The lock is released when the edit ends. It is advisory: an
//! editor that does not take it is caught by the check before publishing,
//! and by the check before undoing.

#[cfg(unix)]
use super::unix::Directory;
#[cfg(windows)]
use super::windows::Directory;
use super::{
    bounded::{read_in, read_text, unreadable},
    place::FilePlace,
};
use crate::resolve::SettingsError;
use std::{
    error,
    ffi::{OsStr, OsString},
    fmt,
    fs::{File, Permissions, TryLockError},
    io::{self, Write as _},
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
};

/// The suffix of the lock file beside a preferences file.
const LOCK: &str = ".lock";

/// The suffix of the recovery copy beside a preferences file.
const PREVIOUS: &str = ".previous";

/// One edit of a preferences file, holding its lock.
#[derive(Debug)]
pub struct FileEdit {
    /// The file's place.
    place: FilePlace,
    /// The file's directory, with the lock file held locked; `None` when
    /// the directory does not exist and was not to be created.
    held: Option<(Directory, File)>,
    /// The file as it was read: its text and permissions.
    before: Option<(String, Permissions)>,
    /// The text published, once it is.
    published: Option<String>,
}

impl FileEdit {
    /// Begins an edit of the file of `place`: opens its directory without
    /// following a link, creating it when `create`, takes the file's lock,
    /// calling `waiting` first when another process holds it, and reads the
    /// file.
    ///
    /// # Errors
    ///
    /// [`FileError::Read`] for a directory or a file that is a link, is not
    /// what it should be, or cannot be read, the lock included.
    pub fn begin(
        place: FilePlace,
        create: bool,
        waiting: impl FnOnce(),
    ) -> Result<Self, FileError> {
        let path = place.path();
        let refused = |error: &io::Error| FileError::Read(unreadable(&path, error));
        let Some(directory) = Directory::open(&place, create).map_err(|error| refused(&error))?
        else {
            return Ok(Self {
                place,
                held: None,
                before: None,
                published: None,
            });
        };
        let lock = directory
            .lock_file(&place.beside(LOCK))
            .map_err(|error| refused(&error))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                waiting();
                lock.lock().map_err(|error| refused(&error))?;
            }
            Err(TryLockError::Error(error)) => return Err(refused(&error)),
        }
        let before = match directory
            .open_regular(place.name())
            .map_err(|error| refused(&error))?
        {
            Some(file) => {
                let permissions = file
                    .metadata()
                    .map_err(|error| refused(&error))?
                    .permissions();
                Some((
                    read_text(file, &path).map_err(FileError::Read)?,
                    permissions,
                ))
            }
            None => None,
        };
        Ok(Self {
            place,
            held: Some((directory, lock)),
            before,
            published: None,
        })
    }

    /// The file's path.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.place.path()
    }

    /// The file's text as it was read, `None` when it did not exist.
    #[must_use]
    pub fn before(&self) -> Option<&str> {
        self.before.as_ref().map(|(text, _)| text.as_str())
    }

    /// The path of the copy of the file as it was, kept beside it while a
    /// published change may still be undone.
    #[must_use]
    pub fn recovery_path(&self) -> PathBuf {
        self.place.directory().join(self.place.beside(PREVIOUS))
    }

    /// Replaces the file by `text`: checks that it still holds what was
    /// read, keeps a recovery copy of it, then writes `text` to a new file,
    /// private until it takes the old file's permissions, and renames it
    /// over the file.
    ///
    /// # Errors
    ///
    /// [`FileError::Changed`] when another program changed the file since
    /// it was read, and [`FileError::Write`] when the new file cannot be
    /// written; the file is as it was either way.
    pub fn publish(&mut self, text: &str) -> Result<(), FileError> {
        let path = self.path();
        let Some((directory, _)) = &self.held else {
            return Err(FileError::Write {
                path,
                reason: "its directory does not exist".to_owned(),
            });
        };
        if read_in(directory, self.place.name(), &path)
            .map_err(FileError::Read)?
            .as_deref()
            != self.before()
        {
            return Err(FileError::Changed(path));
        }
        let failed = |error: io::Error| FileError::Write {
            path: path.clone(),
            reason: error.to_string(),
        };
        let permissions = self.before.as_ref().map(|(_, permissions)| permissions);
        if let Some((before, _)) = &self.before {
            replace(directory, &self.place.beside(PREVIOUS), before, permissions)
                .map_err(failed)?;
        }
        replace(directory, self.place.name(), text, permissions)
            .inspect_err(|_| {
                drop(directory.remove(&self.place.beside(PREVIOUS)));
            })
            .map_err(failed)?;
        self.published = Some(text.to_owned());
        Ok(())
    }

    /// Ends the edit keeping what was published, and removes the recovery
    /// copy; a copy that cannot be removed is left, holding the old text.
    pub fn keep(self) {
        if let Some((directory, _)) = &self.held {
            drop(directory.remove(&self.place.beside(PREVIOUS)));
        }
    }

    /// Ends the edit putting the file back as it was read: the old text, or
    /// no file. It confirms the file holds what it should before and after,
    /// then removes the recovery copy.
    ///
    /// # Errors
    ///
    /// [`RestoreError`] when the file changed since it was published or
    /// cannot be put back, naming the recovery copy when there is one.
    pub fn undo(self) -> Result<(), RestoreError> {
        let path = self.path();
        let (Some((directory, _)), Some(published)) = (&self.held, &self.published) else {
            return Ok(());
        };
        let recovery = self.before.as_ref().map(|_| self.recovery_path());
        let failed = |reason: String| RestoreError {
            path: path.clone(),
            reason,
            recovery: recovery.clone(),
        };
        let read = || {
            read_in(directory, self.place.name(), &path).map_err(|error| failed(error.to_string()))
        };
        if read()?.as_deref() != Some(published.as_str()) {
            return Err(failed("another program changed it since".to_owned()));
        }
        let restored = match &self.before {
            Some((text, permissions)) => {
                replace(directory, self.place.name(), text, Some(permissions))
            }
            None => directory.remove(self.place.name()),
        };
        restored.map_err(|error| failed(error.to_string()))?;
        if read()?.as_deref() != self.before() {
            return Err(failed("it does not read back as it was".to_owned()));
        }
        if recovery.is_some() {
            drop(directory.remove(&self.place.beside(PREVIOUS)));
        }
        Ok(())
    }
}

/// Replaces `name` in `directory` by `text`: written to a new private file,
/// synced, given `permissions` when there are any, and renamed over it. The
/// new file is removed when a step fails.
fn replace(
    directory: &Directory,
    name: &OsStr,
    text: &str,
    permissions: Option<&Permissions>,
) -> io::Result<()> {
    /// Tells apart the files one process writes.
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut temporary = OsString::from(name);
    temporary.push(format!(
        ".tmp-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = directory.create_private(&temporary)?;
    let written = file
        .write_all(text.as_bytes())
        .and_then(|()| {
            permissions.map_or(Ok(()), |permissions| {
                file.set_permissions(permissions.clone())
            })
        })
        .and_then(|()| file.sync_all());
    drop(file);
    written
        .and_then(|()| directory.rename(&temporary, name))
        .inspect_err(|_| {
            drop(directory.remove(&temporary));
        })
}

/// Why an edit of a preferences file did not publish.
#[derive(Debug)]
pub enum FileError {
    /// The file or its directory is a link, is not what it should be, or
    /// cannot be read.
    Read(SettingsError),
    /// Another program changed the file since it was read: nothing was
    /// written.
    Changed(PathBuf),
    /// The new text could not be written: the file is as it was.
    Write {
        /// The file.
        path: PathBuf,
        /// What the operating system reported.
        reason: String,
    },
}

impl FileError {
    /// Whether the error refuses the file, rather than failing to write it.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        !matches!(self, Self::Write { .. })
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "{error}"),
            Self::Changed(path) => write!(
                formatter,
                "{} changed while it was being edited, so nothing was written; run the command \
                 again",
                path.display()
            ),
            Self::Write { path, reason } => {
                write!(formatter, "cannot write {}: {reason}", path.display())
            }
        }
    }
}

impl error::Error for FileError {}

/// Why a published change could not be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreError {
    /// The file.
    pub path: PathBuf,
    /// Why it could not be put back.
    pub reason: String,
    /// The copy of the file as it was, when it existed.
    pub recovery: Option<PathBuf>,
}

impl fmt::Display for RestoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} could not be put back as it was ({})",
            self.path.display(),
            self.reason
        )?;
        match &self.recovery {
            Some(copy) => write!(
                formatter,
                "; its previous text is kept in {}",
                copy.display()
            ),
            None => {
                formatter.write_str("; it did not exist before, so remove it to undo the change")
            }
        }
    }
}

impl error::Error for RestoreError {}
