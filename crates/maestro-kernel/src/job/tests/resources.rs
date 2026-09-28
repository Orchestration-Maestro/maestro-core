//! Resources: what a job holds exclusively while it is queued or running,
//! such as the publication of a collection.

use super::support::{DEMO, FIRST, PUBLISH, Scratch, TERM, at, publish, rows};
use crate::{
    artifact::Digest,
    job::{Error, Job, JobState, NewJob},
    scope::{Scope, ScopeSet},
    store::{Database, Error as StoreError},
};
use serde_json::{Value, json};
use ulid::Ulid;

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

/// Submits and ends one job before the next one holds the same resource.
fn ended(database: &Database, new: &NewJob<'_>, seconds: u64, outcome: JobState) -> Job {
    let job = database.submit_job(new, at(seconds)).unwrap();
    let lease = database.take_job(job.id, FIRST, at(seconds), TERM).unwrap();
    database
        .complete_job(&lease, outcome, &Value::Null)
        .unwrap();
    job
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

#[test]
fn unfinished_jobs_lists_queued_and_running_resource_holders() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let inputs = chunk_set(1);
    let queued = database.submit_job(&holding(&inputs), at(0)).unwrap();
    let scopes = ScopeSet::default_workspace();
    assert_eq!(
        database
            .unfinished_jobs(&scopes, PUBLICATION)
            .unwrap()
            .iter()
            .map(|job| job.id)
            .collect::<Vec<_>>(),
        [queued.id]
    );

    let lease = database.take_job(queued.id, FIRST, at(1), TERM).unwrap();
    let running = database.unfinished_jobs(&scopes, PUBLICATION).unwrap();
    assert_eq!(
        running
            .iter()
            .map(|job| (job.id, job.state))
            .collect::<Vec<_>>(),
        [(queued.id, JobState::Running)]
    );
    database
        .complete_job(&lease, JobState::Failed, &Value::Null)
        .unwrap();
    assert!(
        database
            .unfinished_jobs(&scopes, PUBLICATION)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn jobs_for_resource_filters_by_kind_resource_and_visible_scope() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let outside: Scope = "workspace/other/collection/demo".parse().unwrap();
    let visible_inputs = chunk_set(1);
    let visible_new = NewJob {
        kind: PUBLISH,
        inputs: &visible_inputs,
        scope: &DEMO,
        resource: Some(PUBLICATION),
    };
    let visible = ended(&database, &visible_new, 0, JobState::Failed);

    let wrong_kind_inputs = chunk_set(2);
    let wrong_kind = NewJob {
        kind: "knowledge.verify",
        inputs: &wrong_kind_inputs,
        scope: &DEMO,
        resource: Some(PUBLICATION),
    };
    ended(&database, &wrong_kind, 1, JobState::Failed);

    let wrong_resource_inputs = chunk_set(3);
    let wrong_resource = NewJob {
        kind: PUBLISH,
        inputs: &wrong_resource_inputs,
        scope: &DEMO,
        resource: Some("knowledge.publish/other"),
    };
    ended(&database, &wrong_resource, 2, JobState::Failed);

    let hidden_inputs = chunk_set(4);
    let hidden = NewJob {
        kind: PUBLISH,
        inputs: &hidden_inputs,
        scope: &outside,
        resource: Some(PUBLICATION),
    };
    ended(&database, &hidden, 3, JobState::Failed);

    let jobs = database
        .jobs_for_resource(&ScopeSet::default_workspace(), PUBLISH, PUBLICATION)
        .unwrap();
    assert_eq!(
        jobs.iter().map(|job| job.id).collect::<Vec<_>>(),
        [visible.id]
    );
}

#[test]
fn jobs_for_resource_returns_every_state_newest_first() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let succeeded_inputs = chunk_set(1);
    let succeeded = ended(
        &database,
        &holding(&succeeded_inputs),
        0,
        JobState::Succeeded,
    );
    let failed_inputs = chunk_set(2);
    let failed = ended(&database, &holding(&failed_inputs), 1, JobState::Failed);
    let cancelled = database.submit_job(&holding(&chunk_set(3)), at(2)).unwrap();
    database.cancel_job(cancelled.id, &Value::Null).unwrap();
    let queued = database.submit_job(&holding(&chunk_set(4)), at(3)).unwrap();

    let jobs = database
        .jobs_for_resource(&ScopeSet::default_workspace(), PUBLISH, PUBLICATION)
        .unwrap();
    assert_eq!(
        jobs.iter()
            .map(|job| (job.id, job.state))
            .collect::<Vec<_>>(),
        [
            (queued.id, JobState::Queued),
            (cancelled.id, JobState::Cancelled),
            (failed.id, JobState::Failed),
            (succeeded.id, JobState::Succeeded),
        ]
    );
}

#[test]
fn jobs_for_resource_refuses_a_history_over_its_bound() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .write(|transaction| {
            for sequence in 0..=1000 {
                transaction.execute(
                    "INSERT INTO jobs (id, kind, idempotency_key, attempt, scope, resource,
                                       state, outcome_json)
                     VALUES (?1, ?2, ?3, 1, ?4, ?5, 'failed', 'null')",
                    rusqlite::params![
                        Ulid::from_datetime(at(sequence)).to_string(),
                        PUBLISH,
                        Digest::of(&sequence.to_be_bytes()).as_str(),
                        DEMO.as_str(),
                        PUBLICATION,
                    ],
                )?;
            }
            Ok::<(), StoreError>(())
        })
        .unwrap();

    let error = database
        .jobs_for_resource(&ScopeSet::default_workspace(), PUBLISH, PUBLICATION)
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("resource {PUBLICATION} has more than 1000 visible jobs; refusing its history")
    );
    assert!(
        matches!(
            error,
            Error::TooManyJobs { ref resource, limit: 1000 } if resource == PUBLICATION
        ),
        "{error:?}"
    );
}
