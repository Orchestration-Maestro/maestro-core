//! States: a job moves only forward, and its three outcomes are final.

use super::support::{FIRST, SECOND, Scratch, TERM, at, collection, publish, running};
use crate::job::{Error, Job, JobState};
use serde_json::{Value, json};

/// The three states a job ends in.
const OUTCOMES: [JobState; 3] = [JobState::Succeeded, JobState::Failed, JobState::Cancelled];

#[test]
fn a_running_job_ends_in_each_outcome_with_what_it_reports_and_without_its_lease() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for (index, to) in OUTCOMES.into_iter().enumerate() {
        let (job, lease) = running(&database, &format!("demo-{index}"));
        let outcome = json!({"ended": to.to_string()});
        let ended = database.complete_job(&lease, to, &outcome).unwrap();
        let expected = Job {
            state: to,
            lease: None,
            outcome: Some(outcome),
            ..job.clone()
        };
        assert_eq!(ended, expected);
        assert_eq!(database.job(job.id).unwrap(), Some(expected));
    }
}

#[test]
fn a_queued_job_is_cancelled_without_a_lease_and_a_running_one_only_by_its_holder() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let queued = database
        .submit_job(&publish(&collection("queued")), at(0))
        .unwrap();
    let reason = json!({"reason": "withdrawn"});
    let cancelled = database.cancel_job(queued.id, &reason).unwrap();
    assert_eq!(
        cancelled,
        Job {
            state: JobState::Cancelled,
            outcome: Some(reason.clone()),
            ..queued
        }
    );
    let (job, lease) = running(&database, "running");
    let refused = database.cancel_job(job.id, &reason).unwrap_err();
    assert!(
        matches!(
            &refused,
            Error::Held { job: held, holder, expires }
                if *held == job.id && holder == FIRST && expires == "2026-09-26T12:00:30.000Z"
        ),
        "{refused:?}"
    );
    assert_eq!(
        database.job(job.id).unwrap().unwrap().state,
        JobState::Running
    );
    let ended = database
        .complete_job(&lease, JobState::Cancelled, &reason)
        .unwrap();
    assert_eq!(ended.state, JobState::Cancelled);
    let (abandoned, _) = running(&database, "abandoned");
    let refused = database.cancel_job(abandoned.id, &reason).unwrap_err();
    assert!(
        matches!(&refused, Error::Held { holder, .. } if holder == FIRST),
        "a lease holds its job even once it has expired: {refused:?}"
    );
    let successor = database
        .take_job(abandoned.id, SECOND, at(30), TERM)
        .unwrap();
    let ended = database
        .complete_job(&successor, JobState::Cancelled, &reason)
        .unwrap();
    assert_eq!(ended.state, JobState::Cancelled);
}

#[test]
fn the_three_outcomes_are_final() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let data = json!({"step": 1});
    for (index, to) in OUTCOMES.into_iter().enumerate() {
        let (job, mut lease) = running(&database, &format!("demo-{index}"));
        let ended = database.complete_job(&lease, to, &Value::Null).unwrap();
        let moves = [
            (
                database.take_job(job.id, SECOND, at(60), TERM).unwrap_err(),
                JobState::Running,
            ),
            (
                database.cancel_job(job.id, &Value::Null).unwrap_err(),
                JobState::Cancelled,
            ),
        ];
        for (refused, moved) in moves {
            assert!(
                matches!(
                    &refused,
                    Error::IllegalMove { job: id, from, to: target }
                        if *id == job.id && *from == to && *target == moved
                ),
                "{refused:?}"
            );
        }
        let lost = [
            database
                .complete_job(&lease, JobState::Failed, &Value::Null)
                .unwrap_err(),
            database.heartbeat(&mut lease, at(1), TERM).unwrap_err(),
            database
                .progress(&mut lease, at(1), TERM, &data)
                .unwrap_err(),
        ];
        for refused in lost {
            assert!(matches!(&refused, Error::Lost { .. }), "{refused:?}");
        }
        assert_eq!(database.job(job.id).unwrap(), Some(ended));
    }
}

#[test]
fn a_job_cancelled_while_queued_stays_cancelled() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let job = database
        .submit_job(&publish(&collection("demo")), at(0))
        .unwrap();
    let cancelled = database.cancel_job(job.id, &Value::Null).unwrap();
    let refusals = [
        (
            database.take_job(job.id, FIRST, at(1), TERM).unwrap_err(),
            JobState::Running,
        ),
        (
            database.cancel_job(job.id, &Value::Null).unwrap_err(),
            JobState::Cancelled,
        ),
    ];
    for (refused, moved) in refusals {
        assert!(
            matches!(
                &refused,
                Error::IllegalMove { from: JobState::Cancelled, to, .. } if *to == moved
            ),
            "{refused:?}"
        );
    }
    assert_eq!(database.job(job.id).unwrap(), Some(cancelled));
}

#[test]
fn a_running_job_never_moves_back_to_queued_nor_to_running_again() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, lease) = running(&database, "demo");
    let before = database.job(job.id).unwrap();
    for moved in [JobState::Queued, JobState::Running] {
        let refused = database
            .complete_job(&lease, moved, &Value::Null)
            .unwrap_err();
        assert!(
            matches!(
                &refused,
                Error::IllegalMove { job: id, from: JobState::Running, to }
                    if *id == job.id && *to == moved
            ),
            "{refused:?}"
        );
    }
    assert_eq!(database.job(job.id).unwrap(), before);
}
