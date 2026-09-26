//! What the job tests share: a scratch data directory, the publication they
//! submit, the holders of its leases, the clock they inject, and what they
//! read of the journal and of the tables.

use crate::{
    job::{Job, Lease, NewJob, stream},
    journal::{Event, Filter},
    scope::{Scope, ScopeSet},
    store::Database,
};
use rusqlite::{Connection, types::Value as Stored};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::PathBuf,
    process,
    sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use ulid::Ulid;

/// The kind of the jobs the tests submit.
pub(super) const PUBLISH: &str = "knowledge.publish";
/// The scope of the collection the tests' jobs work on.
pub(super) const SCOPE: &str = "workspace/default/collection/demo";
/// [`SCOPE`] as the scope the tests' jobs are submitted in.
pub(super) static DEMO: LazyLock<Scope> = LazyLock::new(|| SCOPE.parse().unwrap());
/// The holder of the first lease the tests take.
pub(super) const FIRST: &str = "first-process";
/// The holder that takes a lease over.
pub(super) const SECOND: &str = "second-process";
/// How long each lease the tests take or renew holds.
pub(super) const TERM: Duration = Duration::from_secs(30);
/// When the tests' clock starts, 2026-09-26T12:00:00Z, in seconds since the
/// Unix epoch.
const START: u64 = 1_790_424_000;

/// A new empty data directory under the platform's temporary directory,
/// removed with everything in it when dropped: after the databases a test
/// opened in it, which it declares later.
pub(super) struct Scratch(pub(super) PathBuf);

impl Scratch {
    /// A directory of its own for one test.
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-job-{}-{}",
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

/// The time `seconds` after the tests' clock starts.
pub(super) fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(START + seconds)
}

/// The frozen inputs of a publication of `name`: the collection alone.
pub(super) fn collection(name: &str) -> Value {
    json!({ "collection": name })
}

/// A publication with `inputs`, on the tests' scope, holding no resource.
pub(super) fn publish(inputs: &Value) -> NewJob<'_> {
    NewJob {
        kind: PUBLISH,
        inputs,
        scope: &DEMO,
        resource: None,
    }
}

/// A publication of `name` submitted at the start of the tests' clock, and
/// the lease `FIRST` takes on it then.
pub(super) fn running(database: &Database, name: &str) -> (Job, Lease) {
    let job = database
        .submit_job(&publish(&collection(name)), at(0))
        .unwrap();
    let lease = database.take_job(job.id, FIRST, at(0), TERM).unwrap();
    (job, lease)
}

/// Every event of the stream of job `id`, in sequence order.
pub(super) fn journaled(database: &Database, id: Ulid) -> Vec<Event> {
    database
        .events(
            &ScopeSet::default_workspace(),
            &Filter {
                stream: &stream(id),
                after: 0,
                r#type: None,
            },
        )
        .unwrap()
}

/// The types of the events of the stream of job `id`, in sequence order.
pub(super) fn types(database: &Database, id: Ulid) -> Vec<String> {
    journaled(database, id)
        .into_iter()
        .map(|event| event.r#type)
        .collect()
}

/// Every row of `table`, each column as SQLite stores it, in the order the
/// rows were inserted, read through `connection`.
pub(super) fn rows(connection: &Connection, table: &str) -> Vec<Vec<Stored>> {
    let mut statement = connection
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| (0..columns).map(|index| row.get(index)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}
