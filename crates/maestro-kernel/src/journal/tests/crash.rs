//! A crash: the event a process committed before it is read after the
//! reopen, and the one a process was writing when it crashed is not.

use super::support::{ChildProcess, Scratch, imported, whole};
use serde_json::Value;

#[test]
fn a_crash_keeps_the_committed_event_and_loses_the_one_in_progress() {
    let scratch = Scratch::new();
    drop(scratch.open());
    let mut committing = ChildProcess::start("commit", &scratch.0);
    let committed = committing.said();
    committing.crash();
    let mut writing = ChildProcess::start("stop-inside", &scratch.0);
    let in_progress = writing.said();
    writing.crash();
    assert_ne!(committed, in_progress);
    let database = scratch.open();
    let ids: Vec<String> = whole(&database, "crash")
        .iter()
        .map(|event| event.id.to_string())
        .collect();
    assert_eq!(ids, [committed]);
    let next = database
        .record(&imported("crash", "after", &Value::Null))
        .unwrap();
    assert_eq!(next.sequence, 2, "the event lost took no sequence");
}
