//! Connections: one writer shared by every thread, readers of their own, the
//! same settings on each, and where the database keeps its files.

use super::support::{ABC, EMPTY, Scratch, pins, stored};
use crate::{
    artifact::Digest,
    store::{Database, Error, database::link_new_file},
};
use rusqlite::Connection;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::{error, fs, path::Path, sync::Barrier, thread};

/// A table the writers count in.
const COUNTER: (&str, &str) = (
    "0001_counter",
    "CREATE TABLE counter (value INTEGER NOT NULL) STRICT; INSERT INTO counter VALUES (0);",
);

/// The busy timeout in milliseconds and whether foreign keys are enforced.
fn settings(connection: &Connection) -> (i64, i64) {
    let setting = |name| {
        connection
            .pragma_query_value(None, name, |row| row.get(0))
            .unwrap()
    };
    (setting("busy_timeout"), setting("foreign_keys"))
}

/// The names in `directory`, sorted.
fn entries(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort_unstable();
    names
}

/// The journal mode of `connection`.
fn journal_mode(connection: &Connection) -> String {
    connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap()
}

/// How many artifacts `connection` sees recorded.
fn count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT count(*) FROM artifacts", [], |row| row.get(0))
        .unwrap()
}

/// Waits for every other writer, then adds one to the counter 25 times, each
/// time in one transaction that reads the counter before it writes it.
fn count_together(database: &Database, start: &Barrier) {
    start.wait();
    for _ in 0..25 {
        database
            .write(|transaction| {
                let value: i64 =
                    transaction.query_row("SELECT value FROM counter", [], |row| row.get(0))?;
                transaction.execute("UPDATE counter SET value = ?1", [value + 1])?;
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn every_connection_waits_five_seconds_and_enforces_foreign_keys() {
    let scratch = Scratch::new();
    let database = scratch.open();
    assert_eq!(settings(&database.reader().unwrap()), (5000, 1));
    let writer = database
        .write(|transaction| Ok(settings(transaction)))
        .unwrap();
    assert_eq!(writer, (5000, 1));
}

#[test]
fn a_reader_sees_only_commits_and_cannot_write() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .write(|transaction| {
            transaction.execute(
                "INSERT INTO artifacts (digest, bytes, media) VALUES (?1, 3, 'text/plain')",
                [ABC],
            )?;
            assert_eq!(
                count(&database.reader()?),
                0,
                "a write in progress is not seen"
            );
            Ok(())
        })
        .unwrap();
    let reader = database.reader().unwrap();
    assert_eq!(count(&reader), 1);
    assert!(reader.execute("DELETE FROM artifacts", []).is_err());
    assert_eq!(count(&reader), 1, "a reader cannot write");
}

#[test]
fn threads_sharing_the_writer_never_interleave_their_writes() {
    let scratch = Scratch::new();
    let database = scratch.open_with(&[COUNTER]).unwrap();
    let start = Barrier::new(8);
    thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| count_together(&database, &start));
        }
    });
    let value: i64 = database
        .reader()
        .unwrap()
        .query_row("SELECT value FROM counter", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, 200, "every read-modify-write kept its increment");
}

#[test]
fn a_writer_that_panics_is_rolled_back_and_writes_go_on() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let panicked = thread::scope(|scope| {
        scope
            .spawn(|| {
                database.write(|transaction| -> Result<(), Error> {
                    transaction.execute(
                        "INSERT INTO artifacts (digest, bytes, media) VALUES (?1, 0, 'text/plain')",
                        [EMPTY],
                    )?;
                    panic!("a writer stops halfway");
                })
            })
            .join()
            .is_err()
    });
    assert!(panicked);
    assert_eq!(pins(&database, &Digest::parse(EMPTY).unwrap()), None);
    let digest = database.put(b"abc", "text/plain").unwrap();
    assert_eq!(pins(&database, &digest), Some(0));
}

#[test]
fn open_in_keeps_the_database_and_artifacts_in_the_data_directory() {
    let scratch = Scratch::new();
    let data = scratch.0.join("share").join("maestro");
    let database = Database::open_in(&data).unwrap();
    let digest = database.put(b"abc", "text/plain").unwrap();
    assert!(data.join("kernel.sqlite3").is_file());
    let artifact = stored(&data.join("artifacts"), &digest);
    assert_eq!(fs::read(artifact).unwrap(), b"abc");
}

#[test]
fn a_new_database_leaves_no_temporary_file_behind() {
    let scratch = Scratch::new();
    let data = scratch.0.join("maestro");
    let database = Database::open_in(&data).unwrap();
    assert_eq!(
        entries(&data),
        ["kernel.sqlite3", "kernel.sqlite3-shm", "kernel.sqlite3-wal"]
    );
    assert_eq!(journal_mode(&database.reader().unwrap()), "wal");
}

#[test]
fn a_database_another_process_made_first_is_kept_and_the_temporary_removed() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"abc", "text/plain").unwrap();
    let before = entries(&scratch.0);
    link_new_file(&scratch.database()).unwrap();
    assert_eq!(entries(&scratch.0), before, "no temporary name stays");
    assert_eq!(
        pins(&database, &digest),
        Some(0),
        "the database is the first one"
    );
}

#[test]
fn a_database_in_another_journal_mode_is_switched_to_wal() {
    let scratch = Scratch::new();
    let outside = scratch.outside();
    outside
        .execute_batch("CREATE TABLE kept (value INTEGER) STRICT; INSERT INTO kept VALUES (7);")
        .unwrap();
    assert_eq!(journal_mode(&outside), "delete");
    drop(outside);
    let database = scratch.open_with(&[COUNTER]).unwrap();
    let reader = database.reader().unwrap();
    assert_eq!(journal_mode(&reader), "wal");
    let kept: i64 = reader
        .query_row("SELECT value FROM kept", [], |row| row.get(0))
        .unwrap();
    assert_eq!(kept, 7);
}

#[cfg(unix)]
#[test]
fn open_creates_the_database_and_its_directories_for_the_owner_only() {
    let scratch = Scratch::new();
    let data = scratch.0.join("share").join("maestro");
    let database = Database::open_in(&data).unwrap();
    database.put(b"abc", "text/plain").unwrap();
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    for name in ["kernel.sqlite3", "kernel.sqlite3-wal", "kernel.sqlite3-shm"] {
        assert_eq!(mode(&data.join(name)), 0o600, "{name}");
    }
    assert_eq!(mode(&data), 0o700);
    assert_eq!(mode(&scratch.0.join("share")), 0o700);
}

#[test]
fn a_database_path_that_cannot_be_created_is_an_io_error_naming_it() {
    let scratch = Scratch::new();
    let file = scratch.0.join("not-a-directory");
    fs::write(&file, b"").unwrap();
    let database = file.join("kernel.sqlite3");
    let error = Database::open(&database, &scratch.artifacts()).unwrap_err();
    let Error::Io { path, source } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(*path, database);
    assert_eq!(
        error.to_string(),
        format!("cannot access {}", database.display())
    );
    let reason = error::Error::source(&error).map(ToString::to_string);
    assert_eq!(reason, Some(source.to_string()));
}

#[test]
fn a_file_that_is_not_a_database_is_refused_by_sqlite() {
    let scratch = Scratch::new();
    fs::write(
        scratch.database(),
        "a page of text, not of SQLite".repeat(64),
    )
    .unwrap();
    let error = Database::open(&scratch.database(), &scratch.artifacts()).unwrap_err();
    assert!(matches!(error, Error::Sqlite(_)), "{error:?}");
    assert!(error.to_string().contains("kernel database"), "{error}");
    let reason = error::Error::source(&error).map(ToString::to_string);
    assert!(
        reason.is_some_and(|reason| reason.contains("not a database")),
        "{error:?}"
    );
}
