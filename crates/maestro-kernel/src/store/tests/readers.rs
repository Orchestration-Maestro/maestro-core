//! Pool reuse, concurrent units, transaction cleanup and fresh authorization.
use super::support::Scratch;
use crate::{
    scope::Right,
    store::{Database, reader::Reader},
};
use rusqlite::Connection;
use std::{
    fs,
    sync::{Mutex, mpsc},
    thread,
    time::Duration,
};

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
    assert_eq!(
        reader
            .pragma_query_value(None, "query_only", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(
        reader
            .execute_batch("CREATE TEMP TABLE s6_temp(value INTEGER)")
            .is_err()
    );
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
    let renamed = scratch.0.join("renamed.sqlite3");
    fs::rename(scratch.database(), &renamed).unwrap();
    fs::remove_file(&renamed).unwrap();
}

#[test]
fn s6_pool_unlocks_before_open() {
    let pool = Mutex::new(Vec::new());
    let reader = Reader::borrow(&pool, || {
        assert!(pool.try_lock().is_ok(), "pool lock covered connection open");
        Connection::open_in_memory()
    })
    .unwrap();
    drop(reader);
    assert_eq!(pool.lock().unwrap().len(), 1);
}

#[test]
fn s6_failed_rollback_is_unlocked_and_discards_reader() {
    use rusqlite::hooks::{AuthContext, Authorization};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let pool = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::new(AtomicUsize::new(0));
    let reader = Reader::borrow(&pool, Connection::open_in_memory).unwrap();
    reader.execute_batch("BEGIN").unwrap();
    let callback_pool = pool.clone();
    let callback_observed = observed.clone();
    reader
        .authorizer(Some(move |_: AuthContext<'_>| {
            assert!(
                callback_pool.try_lock().is_ok(),
                "pool lock covered SQLite rollback"
            );
            callback_observed.fetch_add(1, Ordering::Relaxed);
            Authorization::Deny
        }))
        .unwrap();
    drop(reader);
    assert!(
        observed.load(Ordering::Relaxed) > 0,
        "rollback authorizer did not run"
    );
    assert!(
        pool.lock().unwrap().is_empty(),
        "failed rollback returned a dirty reader"
    );
    let reader = Reader::borrow(&pool, Connection::open_in_memory).unwrap();
    assert!(reader.is_autocommit());
    assert_eq!(
        reader
            .query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
