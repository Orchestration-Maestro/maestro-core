//! What the unit tests share: a scratch kernel, a job of `synthetic` leased
//! in it, held or lost, and that job's lease as the kernel records it.

use maestro_kernel::{
    job::{Lease, NewJob},
    scope::{Right, ScopeSet},
    store::Database,
};
use serde_json::json;
use std::{
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, SystemTime},
};

/// A new empty directory under the platform's temporary directory for the
/// kernel's data, removed with everything in it when dropped, after the
/// database a test opened in it.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-cli-unit-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The kernel's database in this directory.
    pub(super) fn database(&self) -> Database {
        Database::open_in(&self.0).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// The lease of a new job of `synthetic`, taken by `holder` at `at` for
/// `term`.
pub(super) fn leased(database: &Database, holder: &str, at: SystemTime, term: Duration) -> Lease {
    let inputs = json!({ "test": holder });
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let new = NewJob {
        kind: "knowledge.test",
        inputs: &inputs,
        scope: &scope,
        resource: None,
    };
    let job = database.submit_job(&new, at).unwrap();
    database.take_job(job.id, holder, at, term).unwrap()
}

/// What a principal granted the whole default workspace reads in `database`.
pub(super) fn everything(database: &Database) -> ScopeSet {
    let workspace = "workspace/default".parse().unwrap();
    database
        .grant("tester", &workspace, Right::Read, "test")
        .unwrap();
    database.visible("tester").unwrap()
}

/// The lease of the job of `lease` as the kernel records it now, read
/// through `scopes`.
pub(super) fn recorded(database: &Database, scopes: &ScopeSet, lease: &Lease) -> Lease {
    database
        .job(scopes, lease.job)
        .unwrap()
        .unwrap()
        .lease
        .unwrap()
}

/// The lease of a new job, taken by `holder` two minutes ago for one, and
/// since taken over by a successor: its holder has lost it.
pub(super) fn lost(database: &Database, holder: &str) -> Lease {
    let long_ago = SystemTime::now() - Duration::from_secs(120);
    let lease = leased(database, holder, long_ago, Duration::from_secs(60));
    database
        .take_job(
            lease.job,
            "successor",
            SystemTime::now(),
            Duration::from_secs(60),
        )
        .unwrap();
    lease
}
