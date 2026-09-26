//! Refusals: what each one says, and a stored job the kernel cannot read
//! back.

use super::support::{FIRST, PUBLISH, SCOPE, Scratch, at, collection, publish};
use crate::{
    job::{Error, JobState},
    store,
};
use rusqlite::{params, types::Type};
use serde_json::json;
use std::error;
use ulid::Ulid;

/// The ID the refusals below name.
const ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

#[test]
fn each_refusal_says_what_was_refused_and_why() {
    let job = Ulid::from_string(ID).unwrap();
    let refusals = [
        (Error::UnknownJob(job), format!("no job {ID} is recorded")),
        (
            Error::IllegalMove {
                job,
                from: JobState::Succeeded,
                to: JobState::Running,
            },
            format!(
                "job {ID} cannot move from succeeded to running: a job moves only from queued \
                 to running, then to succeeded, failed or cancelled, or from queued to \
                 cancelled"
            ),
        ),
        (
            Error::Held {
                job,
                holder: FIRST.to_owned(),
                expires: "2026-09-26T12:00:30.000Z".to_owned(),
            },
            format!(
                "job {ID} is leased to first-process with an expiry of \
                 2026-09-26T12:00:30.000Z: only its holder moves the job, and another takes \
                 the lease over only from that time on"
            ),
        ),
        (
            Error::ResourceHeld {
                resource: "knowledge.publish/demo".to_owned(),
                job,
            },
            format!(
                "resource knowledge.publish/demo is held by job {ID}, which is queued or \
                 running: another job on it is refused until that one ends"
            ),
        ),
        (
            Error::Lost {
                job,
                holder: FIRST.to_owned(),
                number: 2,
            },
            format!(
                "lease 2 of first-process on job {ID} was taken over, or its job ended: it \
                 writes nothing more"
            ),
        ),
        (
            Error::Time,
            "a lease is recorded from 1970 to the end of 9999: its time or its expiry falls \
             outside"
                .to_owned(),
        ),
    ];
    for (refusal, message) in refusals {
        assert_eq!(refusal.to_string(), message);
        assert!(error::Error::source(&refusal).is_none(), "{refusal:?}");
    }
    let names: Vec<String> = [
        JobState::Queued,
        JobState::Running,
        JobState::Succeeded,
        JobState::Failed,
        JobState::Cancelled,
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    assert_eq!(
        names,
        ["queued", "running", "succeeded", "failed", "cancelled"]
    );
}

#[test]
fn a_refusal_of_the_journal_or_the_database_reads_as_its_own_with_its_source() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let job = database
        .submit_job(&publish(&collection("demo")), at(0))
        .unwrap();
    let outside = scratch.outside();
    outside.execute_batch("DROP TABLE events").unwrap();
    let journal = database.last_progress(job.id).unwrap_err();
    assert!(matches!(journal, Error::Journal(_)), "{journal:?}");
    outside.execute_batch("DROP TABLE jobs").unwrap();
    let store = database.job(job.id).unwrap_err();
    assert!(matches!(store, Error::Store(_)), "{store:?}");
    for (refusal, table) in [(journal, "events"), (store, "jobs")] {
        assert_eq!(
            refusal.to_string(),
            "the kernel database refused the operation"
        );
        let reason = error::Error::source(&refusal).map(ToString::to_string);
        assert!(
            reason.is_some_and(|reason| reason.contains(&format!("no such table: {table}"))),
            "{refusal:?}"
        );
    }
}

#[test]
fn a_stored_job_the_kernel_cannot_read_back_is_an_error_never_a_guess() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let inputs = json!({"collection": "demo"});
    let key = database
        .submit_job(&publish(&inputs), at(0))
        .unwrap()
        .idempotency_key;
    let outside = scratch.outside();
    // It plants rows the kernel never writes, as a writer that bypasses the
    // table's guards would: without its checks, and deleting each row once
    // read.
    outside
        .execute_batch(
            "DROP TRIGGER jobs_are_never_deleted;
             DELETE FROM jobs;
             PRAGMA ignore_check_constraints = ON;",
        )
        .unwrap();
    // Each row breaks one column: its ID, key, attempt, state, lease number
    // and outcome are columns 0, 2, 3, 6, 7 and 11.
    let rows = [
        ("not a ulid", key.as_str(), 1, "queued", 0, None, 0),
        (ID, "not a digest", 1, "queued", 0, None, 2),
        (ID, key.as_str(), -1, "queued", 0, None, 3),
        (ID, key.as_str(), 1, "paused", 0, None, 6),
        (ID, key.as_str(), 1, "queued", -1, None, 7),
        (ID, key.as_str(), 1, "succeeded", 1, Some("1e400"), 11),
    ];
    for (id, stored_key, attempt, state, number, outcome, column) in rows {
        outside
            .execute(
                "INSERT INTO jobs (id, kind, idempotency_key, attempt, scope, state,
                   lease_number, outcome_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id, PUBLISH, stored_key, attempt, SCOPE, state, number, outcome
                ],
            )
            .unwrap();
        let read = if id == ID {
            database.job(Ulid::from_string(ID).unwrap())
        } else {
            database.submit_job(&publish(&inputs), at(1)).map(Some)
        };
        let refusal = read.unwrap_err();
        assert!(
            matches!(
                &refusal,
                Error::Store(store::Error::Sqlite(
                    rusqlite::Error::FromSqlConversionFailure(found, Type::Text, _)
                        | rusqlite::Error::IntegralValueOutOfRange(found, _)
                )) if *found == column
            ),
            "{refusal:?}"
        );
        outside.execute_batch("DELETE FROM jobs").unwrap();
    }
}
