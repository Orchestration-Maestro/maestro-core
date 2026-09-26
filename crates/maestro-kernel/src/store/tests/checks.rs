//! The checks `maestro doctor` runs on the kernel's own store: SQLite's quick
//! check of the database file, and each recorded artifact present and
//! intact.

use super::support::{Scratch, stored};
use crate::store::{ArtifactCheck, Database};
use std::{
    fs::{self, OpenOptions},
    io::{Seek as _, SeekFrom, Write as _},
};

/// The migration of a table, with a NOT NULL column, that fills many pages
/// of the database file.
const FILLER: [(&str, &str); 1] = [(
    "0001_filler",
    "CREATE TABLE filler (id INTEGER PRIMARY KEY, text TEXT NOT NULL);",
)];

/// The database of `scratch` with its filler table filled, then the last
/// page of its file, a leaf of that table, overwritten from outside.
fn damaged(scratch: &Scratch) -> Database {
    let database = scratch.open_with(&FILLER).unwrap();
    let outside = scratch.outside();
    outside
        .execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 2000)
             INSERT INTO filler (text) SELECT printf('%0200d', i) FROM n;
             PRAGMA wal_checkpoint(TRUNCATE);",
        )
        .unwrap();
    let pages: i64 = outside
        .query_row("PRAGMA page_count", [], |row| row.get(0))
        .unwrap();
    let size: i64 = outside
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .unwrap();
    drop(outside);
    drop(database);
    let mut file = OpenOptions::new()
        .write(true)
        .open(scratch.database())
        .unwrap();
    let last = u64::try_from((pages - 1) * size).unwrap();
    file.seek(SeekFrom::Start(last)).unwrap();
    file.write_all(&vec![0xA5; usize::try_from(size).unwrap()])
        .unwrap();
    drop(file);
    scratch.open_with(&FILLER).unwrap()
}

#[test]
fn an_intact_database_passes_its_quick_check() {
    let scratch = Scratch::new();
    let database = scratch.open();
    assert_eq!(database.quick_check().unwrap(), Vec::<String>::new());
}

#[test]
fn a_damaged_file_fails_the_quick_check_saying_how() {
    let scratch = Scratch::new();
    drop(scratch.open());
    // The header's count of free pages, 5, where the file has none.
    let mut file = OpenOptions::new()
        .write(true)
        .open(scratch.database())
        .unwrap();
    file.seek(SeekFrom::Start(36)).unwrap();
    file.write_all(&5_u32.to_be_bytes()).unwrap();
    drop(file);
    assert_eq!(
        scratch.open().quick_check().unwrap(),
        ["*** in database main ***\nFreelist: size is 0 but should be 5"]
    );
}

#[test]
fn a_damaged_page_that_ends_the_check_fails_it_saying_so() {
    let scratch = Scratch::new();
    assert_eq!(
        damaged(&scratch).quick_check().unwrap(),
        ["database disk image is malformed"],
        "the check of the NOT NULL column reads the damaged page, which ends it"
    );
}

/// Unix alone lets a file be replaced while a connection holds it open;
/// Windows refuses the rename.
#[cfg(unix)]
#[test]
fn a_file_that_is_no_database_is_an_error_not_a_finding() {
    use crate::store::Error;
    use rusqlite::ErrorCode;
    let scratch = Scratch::new();
    let database = scratch.open();
    let other = scratch.0.join("other");
    fs::write(&other, vec![0x5A; 4096]).unwrap();
    for suffix in ["-wal", "-shm"] {
        let mut name = scratch.database().into_os_string();
        name.push(suffix);
        drop(fs::remove_file(name));
    }
    fs::rename(&other, scratch.database()).unwrap();
    let refusal = database.quick_check().unwrap_err();
    assert!(
        matches!(
            &refusal,
            Error::Sqlite(rusqlite::Error::SqliteFailure(failure, _))
                if failure.code == ErrorCode::NotADatabase
        ),
        "{refusal:?}"
    );
}

#[test]
fn every_recorded_artifact_is_checked_present_and_intact() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let abc = database.put(b"abc", "text/plain").unwrap();
    let hello = database.put(b"hello", "text/plain").unwrap();
    let empty = database.put(b"", "text/plain").unwrap();
    let whole = database.put(b"whole", "text/plain").unwrap();
    assert_eq!(
        database.check_artifacts().unwrap(),
        ArtifactCheck {
            recorded: 4,
            missing: Vec::new(),
            damaged: Vec::new(),
        }
    );
    let root = scratch.artifacts();
    fs::remove_file(stored(&root, &hello)).unwrap();
    fs::write(stored(&root, &abc), b"not abc").unwrap();
    fs::remove_file(stored(&root, &empty)).unwrap();
    fs::create_dir(stored(&root, &empty)).unwrap();
    let mut damaged = vec![abc, empty];
    damaged.sort();
    assert_eq!(
        database.check_artifacts().unwrap(),
        ArtifactCheck {
            recorded: 4,
            missing: vec![hello],
            damaged,
        },
        "a file that no longer matches its digest, or is no regular file, is damaged"
    );
    assert_eq!(database.get(&whole).unwrap(), b"whole");
}

#[test]
fn an_empty_store_has_nothing_to_check() {
    let scratch = Scratch::new();
    assert_eq!(
        scratch.open().check_artifacts().unwrap(),
        ArtifactCheck {
            recorded: 0,
            missing: Vec::new(),
            damaged: Vec::new(),
        }
    );
}
