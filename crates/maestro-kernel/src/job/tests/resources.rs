//! Resources: what a job holds exclusively while it is queued or running,
//! such as the publication of a collection.

use super::support::{FIRST, Scratch, TERM, at, publish, rows};
use crate::{
    job::{Error, JobState, NewJob},
    scope::ScopeSet,
};
use serde_json::{Value, json};

/// The resource the tests' publications hold: the publication of the
/// collection `demo`.
const PUBLICATION: &str = "knowledge.publish/demo";

/// The frozen inputs of a publication of chunk set `set` of `demo`: another
/// key for each set.
fn chunk_set(set: u64) -> Value {
    json!({"collection": "demo", "chunk_set": set})
}

/// A publication with `inputs` that holds [`PUBLICATION`].
fn holding(inputs: &Value) -> NewJob<'_> {
    NewJob {
        resource: Some(PUBLICATION),
        ..publish(inputs)
    }
}

#[test]
fn a_second_job_on_a_held_resource_is_refused_naming_the_job_that_holds_it() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let outside = scratch.outside();
    let (first_set, second_set) = (chunk_set(1), chunk_set(2));
    let holder = database.submit_job(&holding(&first_set), at(0)).unwrap();
    assert_eq!(holder.resource.as_deref(), Some(PUBLICATION));
    let refused_while = |seconds, state| {
        assert_eq!(
            database
                .job(&ScopeSet::default_workspace(), holder.id)
                .unwrap()
                .unwrap()
                .state,
            state
        );
        let before = (rows(&outside, "jobs"), rows(&outside, "events"));
        let refused = database
            .submit_job(&holding(&second_set), at(seconds))
            .unwrap_err();
        assert!(
            matches!(
                &refused,
                Error::ResourceHeld { resource, job }
                    if resource == PUBLICATION && *job == holder.id
            ),
            "{refused:?}"
        );
        assert_eq!(
            (rows(&outside, "jobs"), rows(&outside, "events")),
            before,
            "a refused job leaves no row and no event"
        );
    };
    refused_while(1, JobState::Queued);
    let lease = database.take_job(holder.id, FIRST, at(2), TERM).unwrap();
    refused_while(3, JobState::Running);
    assert_eq!(
        database.submit_job(&holding(&first_set), at(4)).unwrap().id,
        holder.id,
        "the key comes first: a retried command finds the job that holds the resource"
    );
    database
        .complete_job(&lease, JobState::Succeeded, &Value::Null)
        .unwrap();
    let next = database.submit_job(&holding(&second_set), at(5)).unwrap();
    assert_ne!(next.id, holder.id);
    assert_eq!(
        (next.state, next.resource.as_deref()),
        (JobState::Queued, Some(PUBLICATION))
    );
}

#[test]
fn a_resource_is_free_again_once_its_job_failed_or_was_cancelled() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let sets = [chunk_set(1), chunk_set(2), chunk_set(3)];
    let failed = database.submit_job(&holding(&sets[0]), at(0)).unwrap();
    let lease = database.take_job(failed.id, FIRST, at(0), TERM).unwrap();
    database
        .complete_job(&lease, JobState::Failed, &Value::Null)
        .unwrap();
    let cancelled = database.submit_job(&holding(&sets[1]), at(1)).unwrap();
    database.cancel_job(cancelled.id, &Value::Null).unwrap();
    let third = database.submit_job(&holding(&sets[2]), at(2)).unwrap();
    assert_eq!(
        (third.state, third.resource.as_deref()),
        (JobState::Queued, Some(PUBLICATION))
    );
}

#[test]
fn jobs_on_other_resources_or_on_none_never_hold_one_another_back() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let sets = [chunk_set(1), chunk_set(2), chunk_set(3), chunk_set(4)];
    let other = NewJob {
        resource: Some("knowledge.publish/other"),
        ..publish(&sets[1])
    };
    let jobs = [
        database.submit_job(&holding(&sets[0]), at(0)).unwrap(),
        database.submit_job(&other, at(1)).unwrap(),
        database.submit_job(&publish(&sets[2]), at(2)).unwrap(),
        database.submit_job(&publish(&sets[3]), at(3)).unwrap(),
    ];
    for job in &jobs {
        assert_eq!(job.state, JobState::Queued, "{job:?}");
    }
    assert_eq!(jobs[2].resource, None);
}
