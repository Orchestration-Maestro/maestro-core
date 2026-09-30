//! Read-only health opens never create or migrate kernel state.

use super::support::Scratch;
use crate::{
    artifact::Digest,
    store::{Database, Error, HealthOpen, open_health_in},
};
use rusqlite::Connection;
use std::fs;

#[test]
fn health_reports_a_missing_kernel_without_creating_it() {
    let scratch = Scratch::new();
    assert!(matches!(
        open_health_in(&scratch.0).unwrap(),
        HealthOpen::Missing
    ));
    assert!(!scratch.database().exists());
    assert!(!scratch.artifacts().exists());
}

#[test]
fn health_reports_old_schema_without_changing_its_file_digest() {
    let scratch = Scratch::new();
    drop(Database::open_before(&scratch.0, "0019_graph_projection").unwrap());
    let before = Digest::of(&fs::read(scratch.database()).unwrap());

    assert!(matches!(
        open_health_in(&scratch.0).unwrap(),
        HealthOpen::NeedsMigration(_)
    ));

    let after = Digest::of(&fs::read(scratch.database()).unwrap());
    assert_eq!(before, after);
}

#[test]
fn health_refuses_an_unknown_migration_without_changing_the_file() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .write(|transaction| {
            transaction.execute(
                "INSERT INTO migrations (name, applied_at) VALUES ('9999_future', 'now')",
                [],
            )?;
            Ok::<_, Error>(())
        })
        .unwrap();
    drop(database);
    let before = Digest::of(&fs::read(scratch.database()).unwrap());

    assert!(matches!(
        open_health_in(&scratch.0),
        Err(Error::UnknownMigration(name)) if name == "9999_future"
    ));

    let after = Digest::of(&fs::read(scratch.database()).unwrap());
    assert_eq!(before, after);
    let connection = Connection::open(scratch.database()).unwrap();
    let count: i64 = connection
        .query_row(
            "SELECT count(*) FROM migrations WHERE name = '9999_future'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}
