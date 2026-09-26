//! A job interrupted mid-way: its first process dies, and a second process
//! takes the lease over once it expires and resumes from the last progress
//! the journal holds.

use super::{
    child::{DATA, DONE, MARK},
    support::{FIRST, SECOND, Scratch, TERM, at, collection, journaled, publish},
};
use crate::job::{Error, JobState};
use serde_json::json;
use std::{
    env,
    io::{BufRead as _, BufReader},
    path::Path,
    process::{Command, Stdio},
};

/// How many steps the job has in all.
const STEPS: u64 = 5;
/// The child test the first process runs, alone.
const CHILD: &str = "job::tests::child::act";

/// Runs this test binary again as the first process, working in the data
/// directory `data`; kills it, as a crash would, once it has said the ID of
/// its job; and returns that ID.
fn first_process(data: &Path) -> String {
    let mut child = Command::new(env::current_exe().unwrap())
        .args([CHILD, "--exact", "--nocapture", "--test-threads=1"])
        .env(DATA, data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let said = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(Result::unwrap)
        .find_map(|line| line.split_once(MARK).map(|(_, words)| words.to_owned()))
        .expect("the first process ended before it said its job");
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(
        !status.success(),
        "the first process ended by itself: {status}"
    );
    said
}

#[test]
fn a_job_interrupted_mid_way_resumes_in_a_second_process_from_its_last_journaled_progress() {
    let scratch = Scratch::new();
    drop(scratch.open());
    let id = first_process(&scratch.0);
    let database = scratch.open();
    let inputs = collection("demo");
    let job = database
        .submit_job(&publish(&inputs), at(DONE + 1))
        .unwrap();
    assert_eq!(job.id.to_string(), id, "the retried command finds the job");
    assert_eq!(job.state, JobState::Running);
    let refused = database
        .take_job(job.id, SECOND, at(DONE + 1), TERM)
        .unwrap_err();
    assert!(
        matches!(
            &refused,
            Error::Held { holder, expires, .. }
                if holder == FIRST && expires == "2026-09-26T12:00:33.000Z"
        ),
        "the last progress renewed the lease: {refused:?}"
    );
    let resumed = at(DONE) + TERM;
    let mut lease = database.take_job(job.id, SECOND, resumed, TERM).unwrap();
    let last = database.last_progress(job.id).unwrap().unwrap();
    let done = last.data["step"].as_u64().unwrap();
    assert_eq!(done, DONE);
    for step in done + 1..=STEPS {
        database
            .progress(&mut lease, resumed, TERM, &json!({ "step": step }))
            .unwrap();
    }
    let ended = database
        .complete_job(&lease, JobState::Succeeded, &json!({ "steps": STEPS }))
        .unwrap();
    assert_eq!(ended.state, JobState::Succeeded);
    let steps: Vec<u64> = journaled(&database, job.id)
        .iter()
        .map(|event| event.data["step"].as_u64().unwrap())
        .collect();
    assert_eq!(steps, [1, 2, 3, 4, 5], "no step is lost nor done twice");
}
