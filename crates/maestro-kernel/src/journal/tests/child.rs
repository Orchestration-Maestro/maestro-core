//! Not a test of its own: what the child processes of the crash and
//! concurrency tests do, as their parent's environment tells them.

use super::support::{ACT, DATA, MARK, RACE, imported};
use crate::{
    journal::event::record,
    store::{Database, Error},
};
use serde_json::Value;
use std::{
    env,
    io::{self, Write as _},
    path::Path,
    process,
};

/// Started by `ChildProcess::start`, does what the variable `ACT` names to
/// the database of the data directory `DATA`; in any other run, nothing.
#[test]
fn act() {
    let Some(act) = env::var_os(ACT) else {
        return;
    };
    let database = Database::open_in(Path::new(&env::var_os(DATA).unwrap())).unwrap();
    match act.to_str().unwrap() {
        "commit" => commit(&database),
        "stop-inside" => stop_inside(&database),
        "race" => race(&database),
        other => panic!("no child does {other}"),
    }
}

/// Records an event on the stream `crash` and says its ID, then waits to be
/// killed.
fn commit(database: &Database) {
    let event = database
        .record(&imported("crash", "committed", &Value::Null))
        .unwrap();
    say(&event.id.to_string());
    wait_for_the_parent();
}

/// Records an event on the stream `crash` inside a write and says its ID,
/// then waits there to be killed before the write commits.
fn stop_inside(database: &Database) {
    database
        .write(|transaction| -> Result<(), Error> {
            let event = record(transaction, &imported("crash", "lost", &Value::Null))?;
            say(&event.id.to_string());
            wait_for_the_parent();
            panic!("the parent closed this child's input instead of killing it");
        })
        .unwrap();
}

/// Says its process ID, then runs `RACE` rounds: in each, it waits for the
/// parent to let it go, records an event on the stream `race` in a write of
/// its own, with that ID as its subject, and says so.
fn race(database: &Database) {
    let subject = process::id().to_string();
    say(&subject);
    for _ in 0..RACE {
        wait_for_the_parent();
        database
            .record(&imported("race", &subject, &Value::Null))
            .unwrap();
        say("recorded");
    }
}

/// Says `words` to the parent: the mark, then the words and a line break.
fn say(words: &str) {
    let mut output = io::stdout().lock();
    writeln!(output, "{MARK}{words}").unwrap();
    output.flush().unwrap();
}

/// Waits for the parent's next line on this child's standard input, or for
/// the parent to close it; a child the parent kills never returns from it.
fn wait_for_the_parent() {
    io::stdin().read_line(&mut String::new()).unwrap();
}
