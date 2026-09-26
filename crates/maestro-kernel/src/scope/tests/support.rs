//! What the scope tests share: a scratch directory for the kernel's data and
//! configuration, scopes by their path, and the events a set reads.

use crate::{
    journal::{Event, Filter, NewEvent},
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use rusqlite::{Connection, params};
use serde_json::Value;
use std::{
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};
use ulid::Ulid;

/// The scope of the collection the tests grant most.
pub(super) const CTM: &str = "workspace/default/collection/ctm";
/// The principal that sees the whole default workspace, to read what the
/// tests record there.
pub(super) const AUDITOR: &str = "auditor";

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: after the databases a test opened in
/// it, which it declares later. It is the kernel's data directory and its
/// configuration directory at once.
pub(super) struct Scratch(pub(super) PathBuf);

impl Scratch {
    /// A directory of its own for one test.
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-scope-{}-{}",
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

    /// Writes `text` as this directory's `config.toml`.
    pub(super) fn configure(&self, text: &str) {
        fs::write(self.0.join("config.toml"), text).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// The scope `path` names.
pub(super) fn scope(path: &str) -> Scope {
    path.parse().unwrap()
}

/// Records on `stream` an import that happened in the scope `path`.
pub(super) fn record_in(database: &Database, stream: &str, path: &str) -> Event {
    database
        .record(&NewEvent {
            stream,
            r#type: "maestro.knowledge.import.completed.v1",
            subject: "import/1",
            scope: path,
            data: &Value::Null,
        })
        .unwrap()
}

/// Records on `stream`, as a program outside the kernel would, an import in
/// the scope `text`: the kernel itself refuses text that is not a scope path.
pub(super) fn record_outside(scratch: &Scratch, stream: &str, text: &str) {
    scratch
        .outside()
        .execute(
            "INSERT INTO events (id, stream, sequence, type, subject, scope, data)
             SELECT ?1, ?2, coalesce(max(sequence), 0) + 1,
                    'maestro.knowledge.import.completed.v1', 'import/1', ?3, 'null'
             FROM events WHERE stream = ?2",
            params![Ulid::generate().to_string(), stream, text],
        )
        .unwrap();
}

/// Every event of `stream` that `scopes` reads, in sequence order.
pub(super) fn read(database: &Database, scopes: &ScopeSet, stream: &str) -> Vec<Event> {
    database
        .events(
            scopes,
            &Filter {
                stream,
                after: 0,
                r#type: None,
            },
        )
        .unwrap()
}

/// What [`AUDITOR`] sees, once granted the whole default workspace.
pub(super) fn audit(database: &Database) -> ScopeSet {
    database
        .grant(AUDITOR, &scope("workspace/default"), Right::Read, "test")
        .unwrap();
    database.visible(AUDITOR).unwrap()
}
