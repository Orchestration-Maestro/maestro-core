//! Changes: each change of a job recorded on its stream of the journal, with
//! what it changed, in the write that makes it.

use super::support::{
    FIRST, SCOPE, SECOND, Scratch, TERM, at, collection, journaled, publish, rows, types,
};
use crate::{
    job::{CANCELLED, CREATED, Error, FAILED, JobState, SUCCEEDED, TAKEN, TAKEN_OVER, stream},
    store::Database,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fmt;
use ulid::Ulid;

/// What the database refuses, from outside the kernel, while a change is
/// tried: first the event it records, then the change of its job's row. Each
/// is the SQL that starts the refusal, then the SQL that ends it.
const REFUSALS: [(&str, &str); 2] = [
    (
        "CREATE TRIGGER no_event BEFORE INSERT ON events
         BEGIN SELECT RAISE(ABORT, 'no event'); END;",
        "DROP TRIGGER no_event;",
    ),
    (
        "CREATE TRIGGER no_insert BEFORE INSERT ON jobs
         BEGIN SELECT RAISE(ABORT, 'no change'); END;
         CREATE TRIGGER no_update BEFORE UPDATE ON jobs
         BEGIN SELECT RAISE(ABORT, 'no change'); END;",
        "DROP TRIGGER no_insert; DROP TRIGGER no_update;",
    ),
];

/// Tries `change` under each of [`REFUSALS`] in turn: each time it fails with
/// the store's refusal, and the jobs and the journal stay as they were. Then
/// it succeeds, recording one event, and what it returns is returned.
fn all_or_nothing<T: fmt::Debug>(outside: &Connection, change: impl Fn() -> Result<T, Error>) -> T {
    let before = (rows(outside, "jobs"), rows(outside, "events"));
    for (refuse, allow) in REFUSALS {
        outside.execute_batch(refuse).unwrap();
        let refused = change().unwrap_err();
        assert!(matches!(refused, Error::Store(_)), "{refused:?}");
        assert_eq!(
            (rows(outside, "jobs"), rows(outside, "events")),
            before,
            "{refuse}"
        );
        outside.execute_batch(allow).unwrap();
    }
    let changed = change().unwrap();
    assert_eq!(
        rows(outside, "events").len(),
        before.1.len() + 1,
        "one event for each change"
    );
    changed
}

/// The type and the data of each event of the stream of job `id`, after
/// checking that each is on that stream, about it, in `SCOPE`, in order.
fn recorded(database: &Database, id: Ulid) -> Vec<(String, Value)> {
    let name = stream(id);
    journaled(database, id)
        .into_iter()
        .zip(1..)
        .map(|(event, sequence)| {
            assert_eq!(
                (
                    event.stream.as_str(),
                    event.subject.as_str(),
                    event.scope.as_str()
                ),
                (name.as_str(), name.as_str(), SCOPE)
            );
            assert_eq!(event.sequence, sequence);
            (event.r#type, event.data)
        })
        .collect()
}

/// The event of `r#type` carrying `data`, as [`recorded`] gives it.
fn event(r#type: &str, data: Value) -> (String, Value) {
    (r#type.to_owned(), data)
}

#[test]
fn each_change_records_its_event_on_the_stream_of_its_job_with_what_it_changed() {
    assert_eq!(
        [CREATED, TAKEN, TAKEN_OVER, SUCCEEDED, FAILED, CANCELLED],
        [
            "maestro.job.created.v1",
            "maestro.job.taken.v1",
            "maestro.job.taken_over.v1",
            "maestro.job.succeeded.v1",
            "maestro.job.failed.v1",
            "maestro.job.cancelled.v1",
        ]
    );
    let scratch = Scratch::new();
    let database = scratch.open();
    let inputs = collection("demo");
    let first = database.submit_job(&publish(&inputs), at(0)).unwrap();
    database.take_job(first.id, FIRST, at(0), TERM).unwrap();
    let taken = database.take_job(first.id, SECOND, at(30), TERM).unwrap();
    let failure = json!({"reason": "the router is unavailable"});
    database
        .complete_job(&taken, JobState::Failed, &failure)
        .unwrap();
    let first_lease = json!({
        "holder": FIRST,
        "number": 1,
        "heartbeat": "2026-09-26T12:00:00.000Z",
        "expires": "2026-09-26T12:00:30.000Z",
    });
    let second_lease = json!({
        "holder": SECOND,
        "number": 2,
        "heartbeat": "2026-09-26T12:00:30.000Z",
        "expires": "2026-09-26T12:01:00.000Z",
    });
    assert_eq!(
        recorded(&database, first.id),
        [
            event(CREATED, json!({"inputs": inputs})),
            event(TAKEN, json!({"lease": first_lease})),
            event(
                TAKEN_OVER,
                json!({"lease": second_lease, "previous_lease": first_lease})
            ),
            event(FAILED, json!({"outcome": failure, "lease": second_lease})),
        ]
    );
    let second = database.submit_job(&publish(&inputs), at(40)).unwrap();
    let withdrawn = json!({"reason": "withdrawn"});
    database.cancel_job(second.id, &withdrawn).unwrap();
    assert_eq!(
        recorded(&database, second.id),
        [
            event(
                CREATED,
                json!({"inputs": inputs, "previous_attempt": first.id.to_string()})
            ),
            event(CANCELLED, json!({"outcome": withdrawn})),
        ]
    );
    let third = database.submit_job(&publish(&inputs), at(50)).unwrap();
    let lease = database.take_job(third.id, FIRST, at(50), TERM).unwrap();
    let result = json!({"generation": 1});
    database
        .complete_job(&lease, JobState::Succeeded, &result)
        .unwrap();
    let third_lease = json!({
        "holder": FIRST,
        "number": 1,
        "heartbeat": "2026-09-26T12:00:50.000Z",
        "expires": "2026-09-26T12:01:20.000Z",
    });
    assert_eq!(
        recorded(&database, third.id),
        [
            event(
                CREATED,
                json!({"inputs": inputs, "previous_attempt": second.id.to_string()})
            ),
            event(TAKEN, json!({"lease": third_lease})),
            event(SUCCEEDED, json!({"outcome": result, "lease": third_lease})),
        ]
    );
}

#[test]
fn each_change_and_its_event_land_in_one_write_or_not_at_all() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let outside = scratch.outside();
    let inputs = collection("demo");
    let job = all_or_nothing(&outside, || database.submit_job(&publish(&inputs), at(0)));
    let lease = all_or_nothing(&outside, || database.take_job(job.id, FIRST, at(0), TERM));
    let taken = all_or_nothing(&outside, || database.take_job(job.id, SECOND, at(30), TERM));
    assert_eq!((lease.number, taken.number), (1, 2));
    all_or_nothing(&outside, || {
        database.complete_job(&taken, JobState::Failed, &Value::Null)
    });
    let retried = all_or_nothing(&outside, || database.submit_job(&publish(&inputs), at(40)));
    all_or_nothing(&outside, || database.cancel_job(retried.id, &Value::Null));
    assert_eq!(
        types(&database, job.id),
        [CREATED, TAKEN, TAKEN_OVER, FAILED]
    );
    assert_eq!(types(&database, retried.id), [CREATED, CANCELLED]);
}
