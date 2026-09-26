//! `job wait <id>`: it follows a job's stream as the job moves, until the
//! job ends, and exits with its outcome; a job the local principal cannot
//! read is unknown to it.

use super::support::Home;
use maestro_kernel::{
    job::{JobState, Lease, NewJob},
    store::Database,
};
use serde_json::json;
use std::time::{Duration, SystemTime};

/// How long the tests' leases last.
const TERM: Duration = Duration::from_secs(60);

/// Submits a job of `synthetic` and takes its lease, as a worker would.
fn running_job(database: &Database) -> Lease {
    let inputs = json!({ "demo": true });
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let new = NewJob {
        kind: "knowledge.demo",
        inputs: &inputs,
        scope: &scope,
        resource: None,
    };
    let job = database.submit_job(&new, SystemTime::now()).unwrap();
    database
        .take_job(job.id, "worker", SystemTime::now(), TERM)
        .unwrap()
}

#[test]
fn job_wait_follows_a_job_to_its_end_and_exits_with_its_outcome() {
    let home = Home::new();
    let database = home.database();
    let mut lease = running_job(&database);
    let mut waiting = home.start(&["job", "wait", &lease.job.to_string()]);
    waiting.line_with("maestro.job.taken.v1");
    let step = json!({ "step": 1 });
    database
        .progress(&mut lease, SystemTime::now(), TERM, &step)
        .unwrap();
    let progressed = waiting.line_with("maestro.job.progressed.v1");
    assert!(progressed.ends_with(&step.to_string()), "{progressed}");
    let outcome = json!({ "done": true });
    database
        .complete_job(&lease, JobState::Succeeded, &outcome)
        .unwrap();
    let ended = waiting.finish();
    assert_eq!(ended.code, Some(0), "{ended:?}");
    assert!(
        ended.stdout.ends_with(&format!("succeeded {outcome}\n")),
        "{ended:?}"
    );
    assert_eq!(ended.stderr, "");
}

#[test]
fn job_wait_exits_one_for_a_job_that_failed() {
    let home = Home::new();
    let database = home.database();
    let lease = running_job(&database);
    let outcome = json!({ "error": "the corpus went away" });
    database
        .complete_job(&lease, JobState::Failed, &outcome)
        .unwrap();
    let ended = home.run(&["job", "wait", &lease.job.to_string(), "--json"]);
    assert_eq!(ended.code, Some(1), "{ended:?}");
    assert_eq!(ended.stderr, "");
    assert_eq!(
        ended.json(),
        json!({
            "schema": "maestro-cli/job-wait/1",
            "job": lease.job.to_string(),
            "kind": "knowledge.demo",
            "attempt": 1,
            "state": "failed",
            "outcome": outcome,
        })
    );
}

#[test]
fn a_job_the_local_principal_cannot_read_is_unknown() {
    let home = Home::new();
    home.configure("[access]\nread = ['workspace/default/collection/other']\n");
    let database = home.database();
    let lease = running_job(&database);
    let ended = home.run(&["job", "wait", &lease.job.to_string()]);
    assert_eq!(ended.code, Some(2), "{ended:?}");
    assert_eq!(ended.stdout, "");
    assert!(ended.stderr.contains(&lease.job.to_string()), "{ended:?}");
}
