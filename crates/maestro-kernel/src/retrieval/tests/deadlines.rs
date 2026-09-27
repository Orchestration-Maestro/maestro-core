//! Cancellation and real-clock bounds on SQLite reads.

use super::support::SearchDb;
use crate::retrieval::{Error, InventoryRequest, ReadControl, SearchRead};
use std::{
    slice,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

#[test]
fn preexpired_and_cancelled_reads_stop_before_opening_sql() {
    let search = SearchDb::new("Install the tool with --force.");
    let expired = ReadControl {
        deadline: Instant::now(),
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
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let reader = super::super::read::controlled_reader(&search.database, &control).unwrap();
    let interrupted = reader.query_row(
        "WITH RECURSIVE counter(value) AS (
           VALUES(1) UNION ALL SELECT value + 1 FROM counter WHERE value < 1000000
         ) SELECT sum(value) FROM counter",
        [],
        |row| row.get::<_, i64>(0),
    );
    assert!(matches!(
        interrupted.map_err(|error| super::super::read::classify(error, &control)),
        Err(Error::TimedOut)
    ));
}
