//! Progress: recorded on the job's stream of the journal in the write that
//! renews its lease, and read back to resume.

use super::support::{SCOPE, Scratch, TERM, at, collection, journaled, publish, running};
use crate::{
    job::{Error, PROGRESSED, stream},
    journal::NewEvent,
};
use serde_json::{Value, json};
use ulid::Ulid;

#[test]
fn progress_goes_to_the_stream_of_its_job_in_the_write_that_renews_the_lease() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, mut lease) = running(&database, "demo");
    let name = format!("job/{}", job.id);
    assert_eq!(stream(job.id), name);
    assert_eq!(PROGRESSED, "maestro.job.progressed.v1");
    let data = json!({"batch": 1, "points": 64});
    let first = database.progress(&mut lease, at(10), TERM, &data).unwrap();
    assert_eq!(
        (
            first.stream.as_str(),
            first.sequence,
            first.r#type.as_str(),
            first.subject.as_str(),
            first.scope.as_str(),
            &first.data,
        ),
        (name.as_str(), 1, PROGRESSED, name.as_str(), SCOPE, &data)
    );
    assert_eq!(
        (lease.heartbeat.as_str(), lease.expires.as_str()),
        ("2026-09-26T12:00:10.000Z", "2026-09-26T12:00:40.000Z")
    );
    assert_eq!(
        database.job(job.id).unwrap().unwrap().lease,
        Some(lease.clone())
    );
    let second = database
        .progress(&mut lease, at(20), TERM, &json!({"batch": 2}))
        .unwrap();
    assert_eq!(second.sequence, 2);
    assert_eq!(lease.expires, "2026-09-26T12:00:50.000Z");
    assert_eq!(journaled(&database, job.id), [first, second.clone()]);
    assert_eq!(database.last_progress(job.id).unwrap(), Some(second));
}

#[test]
fn progress_and_the_renewal_of_its_lease_land_in_one_write_or_not_at_all() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, mut lease) = running(&database, "demo");
    let held = lease.clone();
    let outside = scratch.outside();
    let step = json!({"batch": 1});
    outside
        .execute_batch(
            "CREATE TRIGGER no_event BEFORE INSERT ON events
             BEGIN SELECT RAISE(ABORT, 'no event'); END;",
        )
        .unwrap();
    let refused = database
        .progress(&mut lease, at(10), TERM, &step)
        .unwrap_err();
    assert!(matches!(&refused, Error::Store(_)), "{refused:?}");
    assert_eq!(lease, held, "the holder's lease is unchanged");
    assert_eq!(
        database.job(job.id).unwrap().unwrap().lease,
        Some(held.clone()),
        "the renewal went with the event"
    );
    outside
        .execute_batch(
            "DROP TRIGGER no_event;
             CREATE TRIGGER no_renewal BEFORE UPDATE ON jobs
             BEGIN SELECT RAISE(ABORT, 'no renewal'); END;",
        )
        .unwrap();
    let refused = database
        .progress(&mut lease, at(20), TERM, &step)
        .unwrap_err();
    assert!(matches!(&refused, Error::Store(_)), "{refused:?}");
    assert_eq!(lease, held);
    assert_eq!(
        journaled(&database, job.id),
        [],
        "the event went with the renewal"
    );
    outside.execute_batch("DROP TRIGGER no_renewal").unwrap();
    let recorded = database.progress(&mut lease, at(30), TERM, &step).unwrap();
    assert_eq!(recorded.sequence, 1, "the refused steps took no sequence");
}

#[test]
fn resuming_reads_the_last_progress_of_its_own_job_only() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let job = database
        .submit_job(&publish(&collection("demo")), at(0))
        .unwrap();
    assert_eq!(database.last_progress(job.id).unwrap(), None);
    let mut lease = database.take_job(job.id, "holder", at(0), TERM).unwrap();
    let step = database
        .progress(&mut lease, at(1), TERM, &json!({"step": 1}))
        .unwrap();
    let name = stream(job.id);
    database
        .record(&NewEvent {
            stream: &name,
            r#type: "maestro.job.noted.v1",
            subject: &name,
            scope: SCOPE,
            data: &Value::Null,
        })
        .unwrap();
    let (_, mut other) = running(&database, "other");
    database
        .progress(&mut other, at(2), TERM, &json!({"step": 9}))
        .unwrap();
    assert_eq!(database.last_progress(job.id).unwrap(), Some(step));
    let unknown = Ulid::nil();
    let refused = database.last_progress(unknown).unwrap_err();
    assert!(
        matches!(refused, Error::UnknownJob(id) if id == unknown),
        "{refused:?}"
    );
}
