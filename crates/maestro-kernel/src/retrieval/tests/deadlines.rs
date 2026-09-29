//! Cancellation and clock bounds on SQLite reads.

use super::super::read::{classify, controlled_reader};
use super::support::SearchDb;
use crate::retrieval::{Clock, Error, InventoryRequest, ReadControl, SearchRead, SystemClock};
use rusqlite::Connection;
use std::{
    slice,
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};

/// A clock that reads the instant the test last set.
#[derive(Debug)]
struct ManualClock(Mutex<Instant>);

impl ManualClock {
    fn at(now: Instant) -> Arc<Self> {
        Arc::new(Self(Mutex::new(now)))
    }

    fn set(&self, now: Instant) {
        *self.0.lock().unwrap() = now;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}

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
        deadline: Instant::now() + Duration::from_secs(5),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(true)),
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
        deadline: Instant::now() + Duration::from_secs(5),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(true)),
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
    let control = ReadControl {
        deadline: Instant::now() + Duration::from_millis(20),
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let reader = controlled_reader(&search.database, &control).unwrap();
    let interrupted = long_read(&reader);
    assert!(matches!(
        interrupted.map_err(|error| classify(error, &control)),
        Err(Error::TimedOut)
    ));
}
