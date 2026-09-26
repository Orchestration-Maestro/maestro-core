//! Events: the ID and the sequence recording gives them, the event it
//! returns, and how they are read back.

use super::support::{IMPORTED, SCOPE, Scratch, imported, whole};
use crate::{
    journal::{Error, Event, Filter, NewEvent, event::record},
    scope::{InvalidScope, Scope, ScopeSet},
    store,
};
use rusqlite::types::Type;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    error,
    time::{SystemTime, UNIX_EPOCH},
};

/// The type of the events that hold a revision back.
const HELD: &str = "maestro.knowledge.revision.held.v1";

/// A test's write that fails on purpose; a refusal of the store fails the
/// test instead.
#[derive(Debug)]
struct Stopped;

impl From<store::Error> for Stopped {
    fn from(error: store::Error) -> Self {
        panic!("the store refused the write: {error:?}")
    }
}

/// Milliseconds since the Unix epoch, now.
fn now() -> u64 {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    u64::try_from(elapsed.as_millis()).unwrap()
}

#[test]
fn recording_gives_an_event_a_ulid_and_the_next_sequence_of_its_stream() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let data = json!({"documents": 3});
    let before = now();
    let recorded: Vec<Event> = [
        "collection/a",
        "collection/a",
        "collection/b",
        "collection/a",
    ]
    .into_iter()
    .map(|stream| {
        database
            .record(&imported(stream, "import/1", &data))
            .unwrap()
    })
    .collect();
    let after = now();
    let sequences: Vec<u64> = recorded.iter().map(|event| event.sequence).collect();
    assert_eq!(sequences, [1, 2, 1, 3]);
    let ids: BTreeSet<_> = recorded.iter().map(|event| event.id).collect();
    assert_eq!(ids.len(), 4, "every event has an ID of its own");
    for event in &recorded {
        let made = event.id.timestamp_ms();
        assert!((before..=after).contains(&made), "{event:?}");
    }
    let outside = scratch.outside();
    let mut statement = outside
        .prepare("SELECT id FROM events ORDER BY rowid")
        .unwrap();
    let stored: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let texts: Vec<String> = recorded.iter().map(|event| event.id.to_string()).collect();
    assert_eq!(
        stored, texts,
        "the journal stores each ID as its 26 characters"
    );
    assert!(texts.iter().all(|text| text.len() == 26), "{texts:?}");
}

#[test]
fn record_returns_the_event_as_it_is_stored() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let data = json!({"documents": 3, "held": ["document/7"], "ratio": 0.5});
    let event = database
        .record(&NewEvent {
            stream: "collection/demo",
            r#type: IMPORTED,
            subject: "import/1",
            scope: SCOPE,
            data: &data,
        })
        .unwrap();
    assert_eq!(event.stream, "collection/demo");
    assert_eq!(event.sequence, 1);
    assert_eq!(event.r#type, IMPORTED);
    assert_eq!(event.subject, "import/1");
    assert_eq!(event.scope, SCOPE);
    assert_eq!(event.data, data);
    let time = event.time.as_bytes();
    assert_eq!(time.len(), 24, "{}", event.time);
    assert_eq!([time[4], time[10], time[19], time[23]], *b"-T.Z");
    assert_eq!(whole(&database, "collection/demo"), [event]);
}

#[test]
fn events_are_read_after_a_position_in_sequence_order_and_by_type() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let data = Value::Null;
    let record = |stream, r#type| {
        let event = NewEvent {
            r#type,
            ..imported(stream, "import/1", &data)
        };
        database.record(&event).unwrap()
    };
    let first = record("collection/a", IMPORTED);
    let second = record("collection/a", HELD);
    record("collection/b", IMPORTED);
    let third = record("collection/a", IMPORTED);
    let read = |after, r#type| {
        database
            .events(
                &ScopeSet::default_workspace(),
                &Filter {
                    stream: "collection/a",
                    after,
                    r#type,
                },
            )
            .unwrap()
    };
    let all = [first.clone(), second.clone(), third.clone()];
    assert_eq!(read(0, None), all);
    assert_eq!(read(1, None), [second.clone(), third.clone()]);
    assert_eq!(read(0, Some(IMPORTED)), [first, third]);
    assert_eq!(read(1, Some(HELD)), [second]);
    assert_eq!(read(3, None), []);
    assert_eq!(read(u64::MAX, None), []);
    assert_eq!(whole(&database, "collection/none"), []);
}

/// Whether `refusal` is the store refusing to write `expected` as a scope.
fn refuses_scope(refusal: &store::Error, expected: &InvalidScope) -> bool {
    matches!(
        refusal,
        store::Error::Sqlite(rusqlite::Error::ToSqlConversionFailure(invalid))
            if invalid.downcast_ref::<InvalidScope>() == Some(expected)
    )
}

#[test]
fn an_event_whose_scope_is_no_scope_path_is_refused_before_it_is_written() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for text in [
        "",
        "not a scope",
        "collection/demo",
        "workspace/default/",
        "workspace/default/collection/Demo",
    ] {
        let expected = text.parse::<Scope>().unwrap_err();
        let event = NewEvent {
            scope: text,
            ..imported("collection/a", "import/1", &Value::Null)
        };
        let refusal = database.record(&event).unwrap_err();
        assert!(
            matches!(&refusal, Error::Store(inner) if refuses_scope(inner, &expected)),
            "{text:?}: {refusal:?}"
        );
        let reason = error::Error::source(&refusal).map(ToString::to_string);
        assert_eq!(reason, Some(expected.to_string()));
        let inside = database
            .write(|transaction| record(transaction, &event))
            .unwrap_err();
        assert!(refuses_scope(&inside, &expected), "{text:?}: {inside:?}");
    }
    let recorded: i64 = scratch
        .outside()
        .query_row("SELECT count(*) FROM events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(recorded, 0);
}

#[test]
fn a_write_in_progress_is_not_read_and_reading_inside_it_does_not_wait() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .write(|transaction| {
            record(
                transaction,
                &imported("collection/a", "import/1", &Value::Null),
            )?;
            assert_eq!(whole(&database, "collection/a"), []);
            Ok::<_, store::Error>(())
        })
        .unwrap();
    assert_eq!(whole(&database, "collection/a").len(), 1);
}

#[test]
fn an_event_recorded_in_a_write_that_fails_goes_with_it() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let failed = database.write(|transaction| {
        let event = record(
            transaction,
            &imported("collection/a", "import/1", &Value::Null),
        )?;
        assert_eq!(event.sequence, 1);
        Err::<(), _>(Stopped)
    });
    assert!(matches!(failed, Err(Stopped)), "{failed:?}");
    assert_eq!(whole(&database, "collection/a"), []);
    let next = database
        .record(&imported("collection/a", "import/2", &Value::Null))
        .unwrap();
    assert_eq!(next.sequence, 1, "the event rolled back took no sequence");
}

#[test]
fn a_stored_event_the_journal_cannot_read_back_is_an_error_never_a_guess() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let outside = scratch.outside();
    outside
        .execute_batch(
            "INSERT INTO events (id, stream, sequence, type, subject, scope, data)
             VALUES ('not a ulid', 'collection/a', 1, 'x', 'y', 'workspace/default', '{}');
             INSERT INTO events (id, stream, sequence, type, subject, scope, data)
             VALUES ('01ARZ3NDEKTSV4RRFFQ69G5FAV', 'collection/b', 1, 'x', 'y',
                     'workspace/default', '1e400');",
        )
        .unwrap();
    // The ID is column 0, the data column 7.
    for (stream, column) in [("collection/a", 0), ("collection/b", 7)] {
        let error = database
            .events(
                &ScopeSet::default_workspace(),
                &Filter {
                    stream,
                    after: 0,
                    r#type: None,
                },
            )
            .unwrap_err();
        assert!(
            matches!(
                &error,
                Error::Store(store::Error::Sqlite(
                    rusqlite::Error::FromSqlConversionFailure(found, Type::Text, _)
                )) if *found == column
            ),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            "the kernel database refused the operation"
        );
    }
}
