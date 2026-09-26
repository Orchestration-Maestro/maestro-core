//! The database: its file, one writer connection behind a mutex, and readers
//! on connections of their own.

use super::{
    error::Error,
    migration::{MIGRATIONS, migrate, pending},
};
use crate::{
    artifact::Store,
    filesystem::{create_directories, new_file},
};
use rusqlite::{Connection, ErrorCode, OpenFlags, Transaction, TransactionBehavior};
use std::{
    fs, io,
    path::{self, Path, PathBuf},
    process,
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

/// How long a connection waits for another's lock before it gives up.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// The database's file in the kernel's data directory.
const FILE: &str = "kernel.sqlite3";

/// Numbers the temporary database files of this process, so two opens never
/// share one.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// The kernel's database, and the artifact store whose artifacts it records.
#[derive(Debug)]
pub struct Database {
    /// The one connection that writes, shared by every thread.
    writer: Mutex<Connection>,
    /// The database file, absolute, which each reader opens.
    path: PathBuf,
    /// The artifact store whose artifacts the `artifacts` table records.
    pub(super) artifacts: Store,
}

impl Database {
    /// The database in the file `database`, with its artifacts under
    /// `artifacts`: created, with its missing directories, for the owner
    /// only, then migrated. Relative paths are resolved against the current
    /// directory now; the caller chooses them, and only the caller reads the
    /// environment.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the file or a directory cannot be created,
    /// [`Error::UnknownMigration`] when a newer binary migrated the database,
    /// and [`Error::Sqlite`] when SQLite cannot open or migrate it.
    pub fn open(database: &Path, artifacts: &Path) -> Result<Self, Error> {
        Self::open_with(database, artifacts, MIGRATIONS)
    }

    /// The database of the kernel's data directory `data`, which
    /// [`crate::paths::data_dir`] resolves: `kernel.sqlite3`, with the
    /// artifacts under `artifacts/`.
    ///
    /// # Errors
    ///
    /// As [`Database::open`].
    pub fn open_in(data: &Path) -> Result<Self, Error> {
        Self::open(&data.join(FILE), &data.join("artifacts"))
    }

    /// [`Database::open`] with `migrations` in place of the binary's own.
    pub(super) fn open_with(
        database: &Path,
        artifacts: &Path,
        migrations: &[(&str, &str)],
    ) -> Result<Self, Error> {
        let path = path::absolute(database).map_err(|source| io_error(database, source))?;
        create_file(&path)?;
        // Without SQLite's create flag: a file the link failed to make is an
        // error, never a new database anyone could read.
        let writer = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let mut writer = configured(writer)?;
        // A no-op for a file `open` made; one made elsewhere may use another mode.
        writer.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&mut writer, migrations)?;
        Ok(Self {
            writer: Mutex::new(writer),
            path,
            artifacts: Store::new(artifacts),
        })
    }

    /// Runs `work` in one write transaction on the writer and commits what it
    /// did, or rolls it all back when it fails. The transaction takes the
    /// write lock as it begins, so writers wait for each other, here and in
    /// other processes; keep `work` short, with no file-system work inside.
    ///
    /// `work` must not call a method of this database that writes, `pin` and
    /// `put` among them: the lock is not re-entrant, and the call would wait
    /// forever. It pins and unpins through `artifacts::pin` and
    /// `artifacts::unpin`, which take its transaction. Its error type may be
    /// the caller's own, as long as this module's [`Error`] converts into it,
    /// so that `?` works on the store's calls inside `work`.
    ///
    /// # Errors
    ///
    /// The error of `work`, or [`Error::Sqlite`], converted, when the
    /// transaction cannot begin or commit.
    pub(crate) fn write<T, E: From<Error>>(
        &self,
        work: impl FnOnce(&Transaction<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        // A writer that panicked left no transaction open: dropping it rolled
        // the transaction back, so the connection is sound.
        let mut writer = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        let transaction = writer
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(Error::from)?;
        let value = work(&transaction)?;
        transaction.commit().map_err(Error::from)?;
        Ok(value)
    }

    /// What SQLite's quick check finds wrong in the database file, in its
    /// words: nothing when the file is intact. It reads every page, so it
    /// takes as long as the file is large; `maestro doctor` runs it.
    ///
    /// # Errors
    ///
    /// [`Error::Sqlite`] when the database cannot be read at all.
    pub fn quick_check(&self) -> Result<Vec<String>, Error> {
        let reader = self.reader()?;
        let checked = reader
            .prepare("PRAGMA quick_check")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()
            });
        match checked {
            Ok(found) => Ok(found.into_iter().filter(|line| line != "ok").collect()),
            // A damaged page can end the check itself, as when it reads the
            // rows of a table to check their NOT NULL columns.
            Err(rusqlite::Error::SqliteFailure(failure, message))
                if failure.code == ErrorCode::DatabaseCorrupt =>
            {
                Ok(vec![message.unwrap_or_else(|| failure.to_string())])
            }
            Err(error) => Err(error.into()),
        }
    }

    /// A connection of its own that only reads, and sees the last commit,
    /// never a write in progress.
    ///
    /// # Errors
    ///
    /// [`Error::Sqlite`] when the database file cannot be opened.
    pub(crate) fn reader(&self) -> Result<Connection, Error> {
        configured(Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?)
    }
}

/// The migrations this binary carries that the kernel's database in the data
/// directory `data` does not record, by name, in the order
/// [`Database::open_in`] applies them: none when it is up to date. It opens
/// the file read-only and never creates, migrates or changes it, so that
/// `maestro doctor` and `maestro status`, which ask it before they open a
/// database, never migrate one.
///
/// # Errors
///
/// [`Error::UnknownMigration`] when the database records a migration this
/// binary lacks, which a newer binary applied, and [`Error::Sqlite`] when
/// the file cannot be opened or read, a missing one among them.
pub fn pending_migrations(data: &Path) -> Result<Vec<&'static str>, Error> {
    let connection = configured(Connection::open_with_flags(
        data.join(FILE),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?)?;
    Ok(pending(&connection, MIGRATIONS)?
        .into_iter()
        .map(|(name, _)| name)
        .collect())
}

/// `connection` with the settings every connection of the kernel has: a 5 s
/// wait for another's lock, and foreign keys enforced. rusqlite and the
/// bundled SQLite default to both; the kernel does not depend on it.
pub(super) fn configured(connection: Connection) -> Result<Connection, Error> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    Ok(connection)
}

/// Creates the database file at `path` unless it exists, with its missing
/// directories: for the owner only, since SQLite would create it readable by
/// anyone and gives its `-wal` and `-shm` files the database file's mode, and
/// in WAL mode from the start.
fn create_file(path: &Path) -> Result<(), Error> {
    if path.exists() {
        return Ok(());
    }
    if let Some(directory) = path.parent() {
        create_directories(directory, io_error)?;
    }
    link_new_file(path)
}

/// Makes a database in WAL mode under the temporary name
/// `<path>.tmp-<process>-<number>`, links it to `path`, and removes the
/// temporary name, whether the link was made or another process made the
/// file first. SQLite refuses at once, whatever the busy timeout, a switch to
/// WAL that races another connection's, as when several processes open a new
/// database together; under its temporary name no other connection can reach
/// the file, and the link never replaces one another process made.
///
/// Once linked, the temporary name is a second name of the database: one a
/// crash or a refused removal leaves behind is to be removed, never opened.
pub(super) fn link_new_file(path: &Path) -> Result<(), Error> {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(
        ".tmp-{}-{}",
        process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = PathBuf::from(name);
    let linked =
        make_in_wal_mode(&temporary, path).and_then(|()| link_into_place(&temporary, path));
    drop(fs::remove_file(&temporary));
    linked
}

/// Links `temporary` to `path`. A file another process linked there first is
/// kept, in WAL mode as this one is.
///
/// # Errors
///
/// [`Error::Io`] naming `path` when the link cannot be made for another
/// reason, a file system without hard links among them.
pub(super) fn link_into_place(temporary: &Path, path: &Path) -> Result<(), Error> {
    match fs::hard_link(temporary, path) {
        Err(source) if source.kind() != io::ErrorKind::AlreadyExists => Err(io_error(path, source)),
        _ => Ok(()),
    }
}

/// Creates `temporary` for the owner only, puts it in WAL mode and reads it
/// once, which opens its `-wal` and `-shm` files as any first use would; then
/// closes it before the link, which removes them, so none named after the
/// temporary stays and no connection to it outlives the link. Failures name
/// `path`, the file the caller asked for.
fn make_in_wal_mode(temporary: &Path, path: &Path) -> Result<(), Error> {
    new_file()
        .open(temporary)
        .map_err(|source| io_error(path, source))?;
    let connection = Connection::open(temporary)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))?;
    connection.close().map_err(|(_, error)| Error::from(error))
}

/// An [`Error::Io`] about `path`.
fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}
