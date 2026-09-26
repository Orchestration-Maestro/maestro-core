//! Submitting a job: its ID, its idempotency key, and the job a retried
//! command finds.

use super::support::{FIRST, PUBLISH, SCOPE, Scratch, TERM, at, collection, journaled, publish};
use crate::{
    artifact::Digest,
    job::{Job, JobState, NewJob},
};
use serde_json::json;
use std::time::UNIX_EPOCH;
use ulid::Ulid;

#[test]
fn a_submitted_job_is_queued_under_a_ulid_of_its_time_and_the_digest_of_its_kind_scope_and_inputs()
{
    let scratch = Scratch::new();
    let database = scratch.open();
    let inputs = collection("demo");
    let job = database.submit_job(&publish(&inputs), at(0)).unwrap();
    let millis = at(0).duration_since(UNIX_EPOCH).unwrap().as_millis();
    assert_eq!(
        u128::from(job.id.timestamp_ms()),
        millis,
        "the ID names the millisecond of the injected clock"
    );
    let expected = Job {
        id: job.id,
        kind: PUBLISH.to_owned(),
        idempotency_key: Digest::of(
            br#"["knowledge.publish","workspace/default/collection/demo",{"collection":"demo"}]"#,
        ),
        attempt: 1,
        scope: SCOPE.to_owned(),
        resource: None,
        state: JobState::Queued,
        lease: None,
        outcome: None,
    };
    assert_eq!(job, expected);
    assert_eq!(database.job(job.id).unwrap(), Some(expected));
    assert_eq!(database.job(Ulid::nil()).unwrap(), None);
}

#[test]
fn the_same_kind_scope_and_inputs_make_the_same_key_whatever_the_order_of_their_fields() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let inputs = json!({"collection": "demo", "force": true});
    let reordered = json!({"force": true, "collection": "demo"});
    let job = database.submit_job(&publish(&inputs), at(0)).unwrap();
    assert_eq!(
        database.submit_job(&publish(&reordered), at(1)).unwrap(),
        job
    );
    let prepare = NewJob {
        kind: "knowledge.prepare",
        ..publish(&inputs)
    };
    let elsewhere = NewJob {
        scope: "workspace/default/collection/other",
        ..publish(&inputs)
    };
    let other_inputs = collection("other");
    let others = [
        database.submit_job(&prepare, at(2)).unwrap(),
        database.submit_job(&elsewhere, at(3)).unwrap(),
        database.submit_job(&publish(&other_inputs), at(4)).unwrap(),
    ];
    for other in others {
        assert_ne!(other.idempotency_key, job.idempotency_key, "{other:?}");
        assert_ne!(other.id, job.id);
        assert_eq!((other.attempt, other.state), (1, JobState::Queued));
    }
}

#[test]
fn a_retried_command_returns_the_job_while_it_is_queued_or_running_and_once_it_succeeded() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let inputs = collection("demo");
    let queued = database.submit_job(&publish(&inputs), at(0)).unwrap();
    let retried = |seconds| {
        let before = journaled(&database, queued.id);
        let found = database.submit_job(&publish(&inputs), at(seconds)).unwrap();
        assert_eq!(
            journaled(&database, queued.id),
            before,
            "a retried command records nothing"
        );
        found
    };
    assert_eq!(retried(1), queued);
    let lease = database.take_job(queued.id, FIRST, at(2), TERM).unwrap();
    assert_eq!(
        retried(3),
        Job {
            state: JobState::Running,
            lease: Some(lease.clone()),
            ..queued.clone()
        }
    );
    let outcome = json!({"generation": 1});
    let succeeded = database
        .complete_job(&lease, JobState::Succeeded, &outcome)
        .unwrap();
    assert_eq!(
        succeeded,
        Job {
            state: JobState::Succeeded,
            outcome: Some(outcome),
            ..queued.clone()
        }
    );
    assert_eq!(retried(4), succeeded);
}

#[test]
fn after_a_failure_or_a_cancellation_the_same_key_starts_a_new_attempt() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let inputs = collection("demo");
    let first = database.submit_job(&publish(&inputs), at(0)).unwrap();
    let lease = database.take_job(first.id, FIRST, at(0), TERM).unwrap();
    let failure = json!({"reason": "the router is unavailable"});
    let failed = database
        .complete_job(&lease, JobState::Failed, &failure)
        .unwrap();
    let second = database.submit_job(&publish(&inputs), at(1)).unwrap();
    assert_ne!(second.id, first.id);
    assert_eq!(second.idempotency_key, first.idempotency_key);
    assert_eq!((second.attempt, second.state), (2, JobState::Queued));
    let cancelled = database
        .cancel_job(second.id, &json!({"reason": "withdrawn"}))
        .unwrap();
    let third = database.submit_job(&publish(&inputs), at(2)).unwrap();
    assert_ne!(third.id, second.id);
    assert_eq!((third.attempt, third.state), (3, JobState::Queued));
    assert_eq!(
        database.job(first.id).unwrap(),
        Some(failed),
        "an earlier attempt stays as it ended"
    );
    assert_eq!(database.job(second.id).unwrap(), Some(cancelled));
}
