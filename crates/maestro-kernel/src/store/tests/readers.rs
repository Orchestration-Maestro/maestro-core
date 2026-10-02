//! Pool reuse, concurrent units, transaction cleanup and fresh authorization.
use super::support::Scratch;
use crate::{scope::Right, store::Database};
use std::{sync::mpsc, thread, time::Duration};

#[test]
fn s6_readers_return_and_reuse_without_reconfiguration() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let reader = database.reader().unwrap();
    reader.pragma_update(None, "cache_size", -1777).unwrap();
    assert_eq!(database.reader_opens(), 1);
    drop(reader);
    let reader = database.reader().unwrap();
    let cache: i64 = reader
        .pragma_query_value(None, "cache_size", |row| row.get(0))
        .unwrap();
    assert_eq!(cache, -1777, "the same configured connection is reused");
    assert_eq!(database.reader_opens(), 1);
    assert!(reader.execute("DELETE FROM grants", []).is_err());
}

#[test]
fn s6_concurrent_reader_units_do_not_serialize_queries() {
    let scratch = Scratch::new();
    let database = scratch.open();
    thread::scope(|threads| {
        let (ready, started) = mpsc::channel();
        let mut releases = Vec::new();
        for _ in 0..2 {
            let (release, finished) = mpsc::channel();
            releases.push(release);
            let ready = ready.clone();
            let database = &database;
            threads.spawn(move || {
                let reader = database.reader().unwrap();
                reader
                    .query_row("SELECT 1", [], |row| row.get::<_, i32>(0))
                    .unwrap();
                ready.send(()).unwrap();
                finished.recv_timeout(Duration::from_secs(10)).unwrap();
                drop(reader);
            });
        }
        for _ in 0..2 {
            started.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert_eq!(database.reader_opens(), 2);
        for release in releases {
            release.send(()).unwrap();
        }
    });
    drop(database.reader().unwrap());
    assert_eq!(database.reader_opens(), 2);
}

#[test]
fn s6_reader_return_ends_snapshot_and_rechecks_revoked_grants() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scope = "workspace/default/collection/garden".parse().unwrap();
    database
        .grant("reader", &scope, Right::Read, "owner")
        .unwrap();
    let reader = database.reader().unwrap();
    reader.execute_batch("BEGIN").unwrap();
    assert!(
        Database::visible_on(&reader, "reader")
            .unwrap()
            .covers(&scope)
    );
    database
        .revoke("reader", &scope, Right::Read, "owner")
        .unwrap();
    drop(reader);
    let reader = database.reader().unwrap();
    assert!(reader.is_autocommit());
    assert!(
        !Database::visible_on(&reader, "reader")
            .unwrap()
            .covers(&scope)
    );
    assert_eq!(database.reader_opens(), 1);
}

#[test]
fn s6_idle_readers_close_before_the_writer_final_wal_checkpoint() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database.put(b"synthetic checkpoint", "text/plain").unwrap();
    let reader = database.reader().unwrap();
    reader
        .query_row("PRAGMA freelist_count", [], |row| row.get::<_, i64>(0))
        .unwrap();
    drop(reader);
    let wal = scratch.0.join("kernel.sqlite3-wal");
    let shared = scratch.0.join("kernel.sqlite3-shm");
    assert!(wal.exists());
    drop(database);
    assert!(
        !wal.exists(),
        "idle readers prevented the writer's final checkpoint"
    );
    assert!(!shared.exists());
}
