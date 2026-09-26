//! What the journal tests share: a scratch data directory, the events they
//! record, and this test binary started again as a child process.

use crate::{
    journal::{Event, Filter, NewEvent},
    store::Database,
};
use rusqlite::Connection;
use serde_json::Value;
use std::{
    env, fs,
    io::{BufRead as _, BufReader, Lines, Write as _},
    path::{Path, PathBuf},
    process::{self, Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

/// The type of the events the tests record.
pub(super) const IMPORTED: &str = "maestro.knowledge.import.completed.v1";
/// The scope of the events the tests record.
pub(super) const SCOPE: &str = "workspace/default/collection/demo";
/// How many rounds the children of the race run, each recording one event
/// per round.
pub(super) const RACE: u64 = 25;
/// The environment variable that names what a child does; unset, the child
/// test does nothing.
pub(super) const ACT: &str = "MAESTRO_KERNEL_JOURNAL_TEST_ACT";
/// The environment variable that names the data directory a child opens.
pub(super) const DATA: &str = "MAESTRO_KERNEL_JOURNAL_TEST_DATA";
/// What starts each line a child says, among the test harness's own output.
pub(super) const MARK: &str = "journal-child: ";
/// The test a child runs, alone.
const CHILD: &str = "journal::tests::child::act";

/// A new empty data directory under the platform's temporary directory,
/// removed with everything in it when dropped: after the databases a test
/// opened in it, which it declares later.
pub(super) struct Scratch(pub(super) PathBuf);

impl Scratch {
    /// A directory of its own for one test.
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-journal-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The database of this directory.
    pub(super) fn open(&self) -> Database {
        Database::open_in(&self.0).unwrap()
    }

    /// A connection of the test's own to the database file, which writes as
    /// a program outside the kernel would.
    pub(super) fn outside(&self) -> Connection {
        Connection::open(self.0.join("kernel.sqlite3")).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// An import of `subject` completed, recorded on `stream` with `data`.
pub(super) fn imported<'a>(stream: &'a str, subject: &'a str, data: &'a Value) -> NewEvent<'a> {
    NewEvent {
        stream,
        r#type: IMPORTED,
        subject,
        scope: SCOPE,
        data,
    }
}

/// Every event of `stream` in `database`, in sequence order.
pub(super) fn whole(database: &Database, stream: &str) -> Vec<Event> {
    database
        .events(&Filter {
            stream,
            after: 0,
            r#type: None,
        })
        .unwrap()
}

/// This test binary, started again to run the child test alone: the parent
/// reads what the child says on its standard output, and tells it to go on
/// with a line on its standard input.
pub(super) struct ChildProcess {
    /// The running child.
    process: Child,
    /// Its standard input, which the parent writes to.
    input: ChildStdin,
    /// The lines of its standard output, read as it says them.
    output: Lines<BufReader<ChildStdout>>,
}

impl ChildProcess {
    /// Starts the child that does `act` to the database of the data
    /// directory `data`.
    pub(super) fn start(act: &str, data: &Path) -> Self {
        let mut process = Command::new(env::current_exe().unwrap())
            .args([CHILD, "--exact", "--nocapture"])
            .env(ACT, act)
            .env(DATA, data)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = process.stdin.take().unwrap();
        let output = BufReader::new(process.stdout.take().unwrap()).lines();
        Self {
            process,
            input,
            output,
        }
    }

    /// Lets the child go on, with a line on the input it waits on.
    pub(super) fn tell(&mut self, words: &str) {
        writeln!(self.input, "{words}").unwrap();
        self.input.flush().unwrap();
    }

    /// The next line the child says, without its mark.
    pub(super) fn said(&mut self) -> String {
        self.output
            .by_ref()
            .map(Result::unwrap)
            .find_map(|line| line.strip_prefix(MARK).map(str::to_owned))
            .expect("the child ended before it said anything more")
    }

    /// Closes the child's input, waits for it to end by itself, reading what
    /// it says meanwhile, and tells whether it succeeded.
    pub(super) fn succeeded(self) -> bool {
        let Self {
            mut process,
            input,
            output,
        } = self;
        drop(input);
        for line in output {
            drop(line.unwrap());
        }
        process.wait().unwrap().success()
    }

    /// Kills the child where it waits, as a crash would: nothing it holds is
    /// finished or closed first.
    pub(super) fn crash(mut self) {
        self.process.kill().unwrap();
        let status = self.process.wait().unwrap();
        assert!(!status.success(), "the child ended by itself: {status}");
    }
}
