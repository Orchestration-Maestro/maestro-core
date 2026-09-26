//! Migrations: applied in number order, each once, recorded by name, and a
//! database a newer binary migrated refused before anything changes; and
//! the migrations a database lacks, read without opening it for writing.

use super::support::Scratch;
use crate::store::{
    Error,
    migration::{MIGRATIONS, apply, migrate},
    pending_migrations,
};
use rusqlite::Connection;
use std::{fs, sync::Barrier, thread};

/// Creates the table the other test migrations fill.
const FIRST: (&str, &str) = (
    "0001_first",
    "CREATE TABLE first (value INTEGER NOT NULL) STRICT; INSERT INTO first VALUES (1);",
);
/// Needs `first`, so it fails unless `0001_first` ran before it.
const SECOND: (&str, &str) = ("0002_second", "INSERT INTO first VALUES (2);");
/// Needs `first` too.
const THIRD: (&str, &str) = ("0003_third", "INSERT INTO first VALUES (3);");

/// The names and times `connection` records, in name order.
fn recorded(connection: &Connection) -> Vec<(String, String)> {
    let mut statement = connection
        .prepare("SELECT name, applied_at FROM migrations ORDER BY name")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The names alone.
fn names(connection: &Connection) -> Vec<String> {
    recorded(connection)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

/// The values of `first`, in the order the migrations inserted them.
fn values(connection: &Connection) -> Vec<i64> {
    let mut statement = connection
        .prepare("SELECT value FROM first ORDER BY rowid")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The time SQLite reads from the clock, in the form the migrations record.
fn now(connection: &Connection) -> String {
    connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn a_new_database_is_created_in_wal_mode_and_migrated() {
    let scratch = Scratch::new();
    let clock = Connection::open_in_memory().unwrap();
    let before = now(&clock);
    let database = scratch.open();
    let after = now(&clock);
    let reader = database.reader().unwrap();
    let mode: String = reader
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    let mut expected: Vec<&str> = MIGRATIONS.iter().map(|(name, _)| *name).collect();
    expected.sort_unstable();
    assert_eq!(names(&reader), expected);
    assert!(expected.contains(&"0001_artifacts"), "{expected:?}");
    for (name, applied_at) in recorded(&reader) {
        assert!(
            before <= applied_at && applied_at <= after,
            "{name}: {before} <= {applied_at} <= {after}"
        );
    }
}

#[test]
fn reopening_a_database_applies_nothing_again() {
    let scratch = Scratch::new();
    drop(scratch.open());
    let first = recorded(&scratch.outside());
    drop(scratch.open());
    assert_eq!(recorded(&scratch.outside()), first);
    let scratch = Scratch::new();
    drop(scratch.open_with(&[FIRST]).unwrap());
    drop(scratch.open_with(&[FIRST]).unwrap());
    assert_eq!(values(&scratch.outside()), [1], "0001_first ran once");
}

#[test]
fn databases_opened_at_once_apply_each_migration_once() {
    let scratch = Scratch::new();
    let start = Barrier::new(4);
    let opened: Vec<_> = thread::scope(|scope| {
        let openers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    scratch.open_with(&[FIRST, SECOND, THIRD])
                })
            })
            .collect();
        openers
            .into_iter()
            .map(|opener| opener.join().unwrap())
            .collect()
    });
    assert!(opened.iter().all(Result::is_ok), "{opened:?}");
    let outside = scratch.outside();
    assert_eq!(values(&outside), [1, 2, 3]);
    assert_eq!(names(&outside), ["0001_first", "0002_second", "0003_third"]);
}

#[test]
fn a_migration_another_opener_applied_meanwhile_is_skipped() {
    let scratch = Scratch::new();
    let mut connection = scratch.outside();
    migrate(&mut connection, &[]).unwrap();
    apply(&mut connection, FIRST.0, FIRST.1).unwrap();
    // As an opener that listed the migrations before another applied it.
    apply(&mut connection, FIRST.0, FIRST.1).unwrap();
    assert_eq!(values(&connection), [1]);
    assert_eq!(names(&connection), ["0001_first"]);
}

#[test]
fn migrations_apply_in_number_order_whatever_the_list_order() {
    let scratch = Scratch::new();
    drop(scratch.open_with(&[THIRD, FIRST, SECOND]).unwrap());
    let outside = scratch.outside();
    assert_eq!(values(&outside), [1, 2, 3]);
    assert_eq!(names(&outside), ["0001_first", "0002_second", "0003_third"]);
}

#[test]
fn a_migration_merged_out_of_order_still_applies() {
    let scratch = Scratch::new();
    drop(scratch.open_with(&[FIRST, THIRD]).unwrap());
    drop(scratch.open_with(&[FIRST, SECOND, THIRD]).unwrap());
    let outside = scratch.outside();
    assert_eq!(values(&outside), [1, 3, 2]);
    assert_eq!(names(&outside), ["0001_first", "0002_second", "0003_third"]);
}

#[test]
fn a_database_holding_an_unknown_migration_is_refused_unchanged() {
    let scratch = Scratch::new();
    drop(scratch.open_with(&[FIRST, SECOND]).unwrap());
    let error = scratch.open_with(&[FIRST, THIRD]).unwrap_err();
    assert!(
        matches!(&error, Error::UnknownMigration(name) if name == "0002_second"),
        "{error}"
    );
    let outside = scratch.outside();
    assert_eq!(names(&outside), ["0001_first", "0002_second"]);
    assert_eq!(values(&outside), [1, 2], "0003_third was not applied");
}

#[test]
fn a_failing_migration_leaves_neither_its_changes_nor_its_record() {
    let scratch = Scratch::new();
    let broken = (
        "0002_broken",
        "CREATE TABLE partial (value INTEGER) STRICT; INSERT INTO missing VALUES (1);",
    );
    let error = scratch.open_with(&[FIRST, broken]).unwrap_err();
    assert!(matches!(error, Error::Sqlite(_)), "{error}");
    let outside = scratch.outside();
    assert_eq!(names(&outside), ["0001_first"]);
    let partial: i64 = outside
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name = 'partial'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(partial, 0, "the failed migration's table was rolled back");
    drop(scratch.open_with(&[FIRST, SECOND]).unwrap());
    assert_eq!(values(&outside), [1, 2]);
}

/// The names of `migrations`, in name order.
fn sorted(migrations: &[(&'static str, &str)]) -> Vec<&'static str> {
    let mut names: Vec<&str> = migrations.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    names
}

#[test]
fn the_migrations_a_database_lacks_are_read_without_changing_it() {
    let scratch = Scratch::new();
    drop(scratch.open_with(&MIGRATIONS[..6]).unwrap());
    let lacking = sorted(&MIGRATIONS[6..]);
    assert_eq!(lacking.first(), Some(&"0007_chunk_sets"), "{lacking:?}");
    let before = fs::read(scratch.database()).unwrap();
    assert_eq!(pending_migrations(&scratch.0).unwrap(), lacking);
    assert_eq!(
        fs::read(scratch.database()).unwrap(),
        before,
        "the file is unchanged"
    );
    assert_eq!(
        names(&scratch.outside()),
        sorted(&MIGRATIONS[..6]),
        "nothing was applied"
    );
}

#[test]
fn an_up_to_date_database_lacks_nothing_and_one_never_migrated_lacks_all() {
    let scratch = Scratch::new();
    drop(scratch.open());
    assert_eq!(pending_migrations(&scratch.0).unwrap(), Vec::<&str>::new());
    let scratch = Scratch::new();
    scratch
        .outside()
        .execute_batch("CREATE TABLE other (value INTEGER) STRICT;")
        .unwrap();
    assert_eq!(pending_migrations(&scratch.0).unwrap(), sorted(MIGRATIONS));
}

#[test]
fn a_database_a_newer_binary_migrated_is_named_and_a_missing_one_never_created() {
    let scratch = Scratch::new();
    let missing = pending_migrations(&scratch.0).unwrap_err();
    assert!(matches!(missing, Error::Sqlite(_)), "{missing}");
    assert!(
        !scratch.database().exists(),
        "reading never creates the file"
    );
    drop(scratch.open());
    scratch
        .outside()
        .execute(
            "INSERT INTO migrations (name, applied_at) VALUES ('9999_future', 'now')",
            [],
        )
        .unwrap();
    let newer = pending_migrations(&scratch.0).unwrap_err();
    assert!(
        matches!(&newer, Error::UnknownMigration(name) if name == "9999_future"),
        "{newer}"
    );
}
