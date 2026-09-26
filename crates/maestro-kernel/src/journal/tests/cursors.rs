//! Cursors: where each consumer stands in each stream, moved forward only,
//! and kept across a reopen.

use super::support::{Scratch, imported};
use crate::{journal::Error, store::Database};
use rusqlite::Connection;
use serde_json::Value;
use std::error;

/// A time long before any test runs.
const LONG_AGO: &str = "2000-01-01T00:00:00.000Z";

/// Records `count` events on `stream`.
fn record(database: &Database, stream: &str, count: usize) {
    for _ in 0..count {
        database
            .record(&imported(stream, "import/1", &Value::Null))
            .unwrap();
    }
}

/// The `updated_at` of every cursor `connection` sees.
fn updated(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare("SELECT updated_at FROM cursors")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn a_cursor_starts_at_zero_and_ack_moves_it_forward() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database, "collection/a", 3);
    assert_eq!(database.cursor("indexer", "collection/a").unwrap(), 0);
    database.ack("indexer", "collection/a", 2).unwrap();
    assert_eq!(database.cursor("indexer", "collection/a").unwrap(), 2);
    database.ack("indexer", "collection/a", 2).unwrap();
    assert_eq!(
        database.cursor("indexer", "collection/a").unwrap(),
        2,
        "an ack at the cursor changes nothing"
    );
    database.ack("indexer", "collection/a", 3).unwrap();
    assert_eq!(database.cursor("indexer", "collection/a").unwrap(), 3);
}

#[test]
fn a_cursor_is_written_only_when_its_position_moves() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database, "collection/a", 2);
    let outside = scratch.outside();
    database.ack("indexer", "collection/a", 1).unwrap();
    outside
        .execute("UPDATE cursors SET updated_at = ?1", [LONG_AGO])
        .unwrap();
    database.ack("indexer", "collection/a", 1).unwrap();
    assert_eq!(
        updated(&outside),
        [LONG_AGO],
        "an ack at the cursor writes nothing"
    );
    database.ack("indexer", "collection/a", 2).unwrap();
    let moved = updated(&outside);
    assert_eq!(moved.len(), 1);
    assert_ne!(moved[0], LONG_AGO, "a move is written with its time");
    database.ack("notifier", "collection/a", 0).unwrap();
    assert_eq!(updated(&outside), moved, "an ack at 0 records no cursor");
}

#[test]
fn ack_never_moves_a_cursor_backwards() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database, "collection/a", 3);
    database.ack("indexer", "collection/a", 3).unwrap();
    let error = database.ack("indexer", "collection/a", 2).unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Backwards { consumer, stream, position: 2, current: 3 }
                if consumer == "indexer" && stream == "collection/a"
        ),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "the cursor of indexer on collection/a is at 3: an ack at 2 would move it backwards"
    );
    assert!(error::Error::source(&error).is_none());
    assert_eq!(database.cursor("indexer", "collection/a").unwrap(), 3);
}

#[test]
fn ack_past_the_last_event_of_its_stream_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database, "collection/a", 2);
    let error = database.ack("indexer", "collection/a", 3).unwrap_err();
    assert!(
        matches!(
            &error,
            Error::PastEnd { consumer, stream, position: 3, last: 2 }
                if consumer == "indexer" && stream == "collection/a"
        ),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "collection/a ends at sequence 2: the cursor of indexer cannot move to 3"
    );
    assert!(error::Error::source(&error).is_none());
    assert_eq!(database.cursor("indexer", "collection/a").unwrap(), 0);
    let empty = database.ack("indexer", "collection/b", 1).unwrap_err();
    assert!(matches!(empty, Error::PastEnd { last: 0, .. }), "{empty:?}");
    database.ack("indexer", "collection/b", 0).unwrap();
    database.ack("indexer", "collection/a", 2).unwrap();
    assert_eq!(database.cursor("indexer", "collection/a").unwrap(), 2);
}

#[test]
fn each_consumer_has_a_cursor_of_its_own_on_each_stream() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database, "collection/a", 2);
    record(&database, "collection/b", 1);
    database.ack("indexer", "collection/a", 2).unwrap();
    database.ack("notifier", "collection/a", 1).unwrap();
    database.ack("indexer", "collection/b", 1).unwrap();
    let cursors = [
        ("indexer", "collection/a"),
        ("notifier", "collection/a"),
        ("indexer", "collection/b"),
        ("notifier", "collection/b"),
    ]
    .map(|(consumer, stream)| database.cursor(consumer, stream).unwrap());
    assert_eq!(cursors, [2, 1, 1, 0]);
}

#[test]
fn a_cursor_survives_a_reopen() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database, "collection/a", 2);
    database.ack("indexer", "collection/a", 2).unwrap();
    drop(database);
    let database = scratch.open();
    assert_eq!(database.cursor("indexer", "collection/a").unwrap(), 2);
    let updated: String = scratch
        .outside()
        .query_row("SELECT updated_at FROM cursors", [], |row| row.get(0))
        .unwrap();
    assert!(updated.ends_with('Z') && updated.len() == 24, "{updated}");
}

#[test]
fn a_refusal_of_the_database_reads_as_its_own_with_its_source() {
    let scratch = Scratch::new();
    let database = scratch.open();
    scratch
        .outside()
        .execute_batch("DROP TABLE cursors")
        .unwrap();
    let error = database.cursor("indexer", "collection/a").unwrap_err();
    assert!(matches!(error, Error::Store(_)), "{error:?}");
    assert_eq!(
        error.to_string(),
        "the kernel database refused the operation"
    );
    let reason = error::Error::source(&error).map(ToString::to_string);
    assert!(
        reason.is_some_and(|reason| reason.contains("no such table: cursors")),
        "{error:?}"
    );
}
