//! Writers together: threads sharing the database and processes of their own
//! never skip or repeat a sequence of the stream they record on.

use super::support::{ChildProcess, RACE, Scratch, imported, whole};
use crate::{journal::Event, store::Database};
use serde_json::Value;
use std::{collections::BTreeSet, sync::Barrier, thread};

/// The sequences of `events`, and how many IDs they hold between them.
fn sequences_and_ids(events: &[Event]) -> (Vec<u64>, usize) {
    let sequences = events.iter().map(|event| event.sequence).collect();
    let ids: BTreeSet<_> = events.iter().map(|event| event.id).collect();
    (sequences, ids.len())
}

/// Waits for every other thread, then records 25 events on the stream `race`,
/// each in a write of its own.
fn record_together(database: &Database, start: &Barrier) {
    start.wait();
    for _ in 0..25 {
        database
            .record(&imported("race", "thread", &Value::Null))
            .unwrap();
    }
}

#[test]
fn threads_recording_together_never_skip_or_repeat_a_sequence() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let start = Barrier::new(8);
    thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| record_together(&database, &start));
        }
    });
    let (sequences, ids) = sequences_and_ids(&whole(&database, "race"));
    assert_eq!(sequences, (1..=200).collect::<Vec<u64>>());
    assert_eq!(ids, 200);
}

#[test]
fn processes_recording_together_never_skip_or_repeat_a_sequence() {
    let scratch = Scratch::new();
    drop(scratch.open());
    let mut children = [
        ChildProcess::start("race", &scratch.0),
        ChildProcess::start("race", &scratch.0),
    ];
    let processes: BTreeSet<String> = children.iter_mut().map(ChildProcess::said).collect();
    // Each round lets both children record at once, then waits for both.
    for _ in 0..RACE {
        for child in &mut children {
            child.tell("go");
        }
        for child in &mut children {
            assert_eq!(child.said(), "recorded");
        }
    }
    for child in children {
        assert!(child.succeeded());
    }
    let database = scratch.open();
    let events = whole(&database, "race");
    let (sequences, ids) = sequences_and_ids(&events);
    assert_eq!(sequences, (1..=2 * RACE).collect::<Vec<u64>>());
    assert_eq!(ids, events.len());
    for round in events.chunks(2) {
        let subjects: BTreeSet<String> = round.iter().map(|event| event.subject.clone()).collect();
        assert_eq!(subjects, processes, "one event of each process a round");
    }
}
