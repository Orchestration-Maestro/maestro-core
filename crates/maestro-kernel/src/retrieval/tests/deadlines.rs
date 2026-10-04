//! Cancellation and clock bounds on SQLite reads.

use super::super::read::{classify, controlled_reader};
use super::{
    clock::{ManualClock, control},
    support::SearchDb,
};
use crate::retrieval::{Error, InventoryRequest, ReadControl, SearchRead, SystemClock};
use rusqlite::Connection;
use std::{
    slice,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// The last instant before `deadline`: a read may still run.
fn just_before(deadline: Instant) -> Instant {
    deadline.checked_sub(Duration::from_nanos(1)).unwrap()
}

/// A live control whose `clock` alone decides whether `deadline` passed.
fn control_on(deadline: Instant, clock: Arc<ManualClock>) -> ReadControl {
    ReadControl {
        deadline,
        clock,
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}

/// Sums a million generated rows: far more SQLite operations than one
/// progress-handler interval.
fn long_read(reader: &Connection) -> rusqlite::Result<i64> {
    reader.query_row(
        "WITH RECURSIVE counter(value) AS (
           VALUES(1) UNION ALL SELECT value + 1 FROM counter WHERE value < 1000000
         ) SELECT sum(value) FROM counter",
        [],
        |row| row.get::<_, i64>(0),
    )
}

#[test]
fn read_control_times_out_exactly_when_its_clock_reaches_the_deadline() {
    let deadline = Instant::now() + Duration::from_secs(3600);
    let before = control_on(deadline, ManualClock::at(just_before(deadline)));
    assert!(before.check().is_ok());

    let at = control_on(deadline, ManualClock::at(deadline));
    assert!(matches!(at.check(), Err(Error::TimedOut)));
}

#[test]
fn progress_handler_interrupts_once_the_clock_reaches_the_deadline() {
    let search = SearchDb::new("Install the tool with --force.");
    let deadline = Instant::now() + Duration::from_secs(3600);
    let clock = ManualClock::at(just_before(deadline));
    let control = control_on(deadline, clock.clone());
    let reader = controlled_reader(&search.database, &control).unwrap();
    assert!(long_read(&reader).is_ok());

    clock.set(deadline);
    let interrupted = long_read(&reader);
    assert!(matches!(
        interrupted.map_err(|error| classify(error, &control)),
        Err(Error::TimedOut)
    ));
}

#[test]
fn a_failed_read_is_timed_out_exactly_when_its_clock_reaches_the_deadline() {
    let deadline = Instant::now() + Duration::from_secs(3600);
    let clock = ManualClock::at(just_before(deadline));
    let control = control_on(deadline, clock.clone());
    assert!(matches!(
        classify(rusqlite::Error::QueryReturnedNoRows, &control),
        Error::Store(_)
    ));

    clock.set(deadline);
    assert!(matches!(
        classify(rusqlite::Error::QueryReturnedNoRows, &control),
        Error::TimedOut
    ));
}

#[test]
fn read_control_rejects_deadline_and_cancellation() {
    let expired = ReadControl {
        deadline: Instant::now(),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    assert!(matches!(expired.check(), Err(Error::TimedOut)));

    let cancelled = ReadControl {
        cancelled: Arc::new(AtomicBool::new(true)),
        ..control()
    };
    assert!(matches!(cancelled.check(), Err(Error::Cancelled)));
}

#[test]
fn preexpired_and_cancelled_reads_stop_before_opening_sql() {
    let search = SearchDb::new("Install the tool with --force.");
    let expired = ReadControl {
        deadline: Instant::now(),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let read = SearchRead {
        generation: &search.generation,
        scopes: &search.scopes,
        version: None,
        control: &expired,
    };
    assert!(matches!(
        search
            .database
            .search_chunks(&read, slice::from_ref(&search.chunk_id)),
        Err(Error::TimedOut)
    ));

    let cancelled = ReadControl {
        cancelled: Arc::new(AtomicBool::new(true)),
        ..control()
    };
    let read = SearchRead {
        control: &cancelled,
        ..read
    };
    assert!(matches!(
        search
            .database
            .inventory(&read, &InventoryRequest::DocumentsBySet { set: None }),
        Err(Error::Cancelled)
    ));
}

#[test]
fn progress_handler_interrupts_a_long_recursive_read() {
    let search = SearchDb::new("Install the tool with --force.");
    let deadline = Instant::now() + Duration::from_millis(20);
    let clock = ManualClock::at(just_before(deadline));
    let control = control_on(deadline, clock.clone());
    let reader = controlled_reader(&search.database, &control).unwrap();
    // The first SQLite progress callback runs before expiry; the second expires.
    clock.advance_after_reads(1, deadline);
    let interrupted = long_read(&reader);
    assert!(matches!(
        interrupted.map_err(|error| classify(error, &control)),
        Err(Error::TimedOut)
    ));
}

#[test]
fn s6_cancelled_controlled_reader_cannot_change_or_enter_pool() {
    let search = SearchDb::new("Synthetic controlled read.");
    let pooled = search.database.reader().unwrap();
    pooled.pragma_update(None, "cache_size", -1777).unwrap();
    drop(pooled);
    let before = search.database.reader_opens();
    let control = control();
    let controlled: Connection = controlled_reader(&search.database, &control).unwrap();
    control.cancelled.store(true, Ordering::Relaxed);
    assert!(long_read(&controlled).is_err());
    drop(controlled);
    assert_eq!(search.database.reader_opens(), before + 1);
    let pooled = search.database.reader().unwrap();
    assert!(
        long_read(&pooled).is_ok(),
        "no cancelled callback reaches the pool"
    );
    let cache: i64 = pooled
        .pragma_query_value(None, "cache_size", |row| row.get(0))
        .unwrap();
    assert_eq!(
        cache, -1777,
        "controlled reads did not borrow the idle connection"
    );
    let timeout: i64 = pooled
        .pragma_query_value(None, "busy_timeout", |row| row.get(0))
        .unwrap();
    assert_eq!(timeout, 5000);
    drop(pooled);
    let first = search.database.reader().unwrap();
    let second = search.database.reader().unwrap();
    assert_eq!(
        search.database.reader_opens(),
        before + 2,
        "search did not add an idle connection"
    );
    drop((first, second));
}

#[test]
fn s6_controlled_query_and_pooled_lookup_overlap() {
    use std::{sync::mpsc, thread};
    let search = SearchDb::new("Synthetic concurrent search and scoped lookup.");
    let database = &search.database;
    let (ready, started) = mpsc::channel();
    let (release, finished) = mpsc::channel::<()>();
    let mut finished = Some(finished);
    // Pauses the controlled SQL call once, inside SQLite, until the lookup ran.
    let pause = move || {
        if let Some(finished) = finished.take() {
            ready.send(()).unwrap();
            finished.recv_timeout(Duration::from_secs(10)).unwrap();
        }
        false
    };
    thread::scope(|threads| {
        let query = threads.spawn(move || {
            let control = control();
            let reader = controlled_reader(database, &control).unwrap();
            reader.progress_handler(1, Some(pause)).unwrap();
            reader
                .query_row("SELECT 1", [], |row| row.get::<_, i32>(0))
                .unwrap()
        });
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        let (done, visible) = mpsc::channel();
        let lookup = threads.spawn(move || {
            done.send(database.visible("reader").unwrap()).unwrap();
        });
        let result = visible.recv_timeout(Duration::from_secs(5));
        release.send(()).unwrap();
        assert!(
            !result.unwrap().is_empty(),
            "scoped lookup did not overlap the controlled SQL call"
        );
        assert_eq!(query.join().unwrap(), 1);
        lookup.join().unwrap();
    });
}
