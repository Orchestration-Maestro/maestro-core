//! Not a test of its own: the first process of the resume test, which works
//! on a job until its parent kills it.

use super::support::{FIRST, TERM, at, collection, publish};
use crate::store::Database;
use serde_json::json;
use std::{
    env,
    io::{self, Write as _},
    path::Path,
};

/// The environment variable that names the data directory the child opens;
/// unset, the child test does nothing.
pub(super) const DATA: &str = "MAESTRO_KERNEL_JOB_TEST_DATA";
/// What comes before what the child says, among its test harness's own
/// output, which may precede it on the same line.
pub(super) const MARK: &str = "job-child: ";
/// How many steps the first process records before it is killed.
pub(super) const DONE: u64 = 3;

/// Started by the resume test in the data directory `DATA`: submits the
/// publication of `demo`, takes its lease as `FIRST` at the start of the
/// clock, records its first `DONE` steps one second apart, says the job's ID,
/// then waits for its parent to kill it mid-way. In any other run, nothing.
#[test]
fn act() {
    let Some(data) = env::var_os(DATA) else {
        return;
    };
    let database = Database::open_in(Path::new(&data)).unwrap();
    let job = database
        .submit_job(&publish(&collection("demo")), at(0))
        .unwrap();
    let mut lease = database.take_job(job.id, FIRST, at(0), TERM).unwrap();
    for step in 1..=DONE {
        database
            .progress(&mut lease, at(step), TERM, &json!({ "step": step }))
            .unwrap();
    }
    let mut output = io::stdout().lock();
    writeln!(output, "{MARK}{}", job.id).unwrap();
    output.flush().unwrap();
    io::stdin().read_line(&mut String::new()).unwrap();
}
