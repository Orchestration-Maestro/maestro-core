//! Superseding the job that holds the resource an import needs: one whose
//! lease expired, or that no process took, is cancelled, superseded by the
//! import's inputs; one a live lease holds is left alone; and one that ended
//! meanwhile leaves the resource free, as it ended.

use super::support::{Scratch, everything, leased};
use crate::cli::foreground::supersede;
use maestro_kernel::job::{self, JobState, NewJob};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime};
use ulid::Ulid;

/// The inputs of the import that supersedes the holder.
fn inputs() -> Value {
    json!({ "collection": "synthetic", "manifests": { "handbook": "newer" } })
}

/// The outcome the superseded holder ends with.
fn superseded() -> Value {
    json!({ "superseded_by": { "inputs": inputs() } })
}

#[test]
fn a_holder_whose_lease_expired_is_cancelled_superseded_by_the_new_inputs() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let long_ago = SystemTime::now() - Duration::from_secs(120);
    let stale = leased(&database, "crashed", long_ago, Duration::from_secs(60));
    assert!(supersede(&database, stale.job, &inputs()).unwrap());
    let ended = database
        .job(&everything(&database), stale.job)
        .unwrap()
        .unwrap();
    assert_eq!(
        (ended.state, ended.outcome),
        (JobState::Cancelled, Some(superseded()))
    );
}

#[test]
fn a_holder_no_process_took_is_superseded_too() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let older = json!({ "collection": "synthetic", "manifests": { "handbook": "older" } });
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let queued = database
        .submit_job(
            &NewJob {
                kind: "knowledge.import",
                inputs: &older,
                scope: &scope,
                resource: Some("collection/synthetic/import"),
            },
            SystemTime::now(),
        )
        .unwrap();
    assert!(supersede(&database, queued.id, &inputs()).unwrap());
    let ended = database
        .job(&everything(&database), queued.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        (ended.state, ended.outcome),
        (JobState::Cancelled, Some(superseded()))
    );
}

#[test]
fn a_holder_a_live_lease_holds_is_left_alone() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let live = leased(
        &database,
        "another",
        SystemTime::now(),
        Duration::from_secs(60),
    );
    assert!(!supersede(&database, live.job, &inputs()).unwrap());
    let kept = database
        .job(&everything(&database), live.job)
        .unwrap()
        .unwrap();
    assert_eq!(
        (kept.state, kept.lease),
        (JobState::Running, Some(live)),
        "it runs on under its lease"
    );
}

#[test]
fn a_holder_that_ended_meanwhile_leaves_the_resource_free_as_it_ended() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let lease = leased(
        &database,
        "another",
        SystemTime::now(),
        Duration::from_secs(60),
    );
    let outcome = json!({ "imported": 1 });
    database
        .complete_job(&lease, JobState::Succeeded, &outcome)
        .unwrap();
    assert!(supersede(&database, lease.job, &inputs()).unwrap());
    let kept = database
        .job(&everything(&database), lease.job)
        .unwrap()
        .unwrap();
    assert_eq!(
        (kept.state, kept.outcome),
        (JobState::Succeeded, Some(outcome))
    );
}

#[test]
fn a_holder_the_kernel_does_not_know_is_an_error() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let unknown = supersede(&database, Ulid::nil(), &inputs());
    assert!(
        matches!(unknown, Err(job::Error::UnknownJob(_))),
        "{unknown:?}"
    );
}
