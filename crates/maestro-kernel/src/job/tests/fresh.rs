//! Fresh-only attempts never mutate or take over a job found by its key.
use super::support::{FIRST, Scratch, TERM, at, collection, journaled, publish, rows};
use crate::job::{Error, JobState, NewJob, Submitted};
use serde_json::json;

#[test]
fn fresh_submission_finds_queued_running_and_succeeded_without_writes() {
    let scratch = Scratch::new();
    let db = scratch.open();
    let inputs = collection("demo");
    let new = publish(&inputs);
    let Submitted::Created(queued) = db.submit_new_job(&new, at(0)).unwrap() else {
        panic!("first submission must create an attempt");
    };
    for expected in [JobState::Queued, JobState::Running, JobState::Succeeded] {
        let before = journaled(&db, queued.id);
        let jobs = rows(&scratch.outside(), "jobs");
        let Submitted::Found(found) = db.submit_new_job(&new, at(100)).unwrap() else {
            panic!("fresh submission must not reuse existing work");
        };
        assert_eq!(found.id, queued.id);
        assert_eq!(found.state, expected);
        assert_eq!(journaled(&db, queued.id), before);
        assert_eq!(rows(&scratch.outside(), "jobs"), jobs);
        if expected == JobState::Queued {
            db.take_job(queued.id, FIRST, at(0), TERM).unwrap();
        } else if expected == JobState::Running {
            db.complete_job(&found.lease.unwrap(), JobState::Succeeded, &json!({}))
                .unwrap();
        }
    }
}

#[test]
fn fresh_submission_creates_attempts_after_failure_and_cancellation() {
    let scratch = Scratch::new();
    let db = scratch.open();
    let inputs = collection("demo");
    let new = publish(&inputs);
    for (attempt, ending) in [(1, JobState::Failed), (2, JobState::Cancelled)] {
        let Submitted::Created(job) = db.submit_new_job(&new, at(attempt)).unwrap() else {
            panic!("terminal attempt must permit fresh work");
        };
        assert_eq!(job.attempt, attempt);
        let lease = db.take_job(job.id, FIRST, at(attempt), TERM).unwrap();
        db.complete_job(&lease, ending, &json!({})).unwrap();
    }
    let Submitted::Created(job) = db.submit_new_job(&new, at(3)).unwrap() else {
        panic!("cancelled attempt must permit fresh work");
    };
    assert_eq!((job.attempt, job.state), (3, JobState::Queued));
}

#[test]
fn fresh_submission_refuses_held_resource_without_superseding() {
    let scratch = Scratch::new();
    let db = scratch.open();
    let inputs = collection("demo");
    let other = collection("other");
    let new = NewJob {
        resource: Some("shared"),
        ..publish(&inputs)
    };
    let holder = db.submit_job(&new, at(0)).unwrap();
    let before = rows(&scratch.outside(), "jobs");
    let events = journaled(&db, holder.id);
    assert!(
        matches!(db.submit_new_job(&NewJob { inputs: &other, ..new }, at(100)),
        Err(Error::ResourceHeld { job, .. }) if job == holder.id)
    );
    assert_eq!(rows(&scratch.outside(), "jobs"), before);
    assert_eq!(journaled(&db, holder.id), events);
}
