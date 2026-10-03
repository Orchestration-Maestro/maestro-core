//! Independent processes synchronize over pipes, never sleeps. Native Windows removal
//! stays refused; Unix tests prove the permanent access domain independent of lbug locks.
use super::super::cleanup::{Cleanup, CleanupError, CleanupOutcome};
use super::cleanup_support::{Fixture, timing};
use maestro_filesystem::{ControlFile, LockMode, OwnedRoot, SystemFileLock};
use maestro_kernel::{job::JobState, store::Database};
use serde_json::json;
use std::{
    env,
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::Duration,
};

/// Pipe-synchronized actor used only by the parent tests below.
#[test]
fn cleanup_process_actor() {
    let Ok(path) = env::var("MAESTRO_E10_PROCESS_ROOT") else {
        return;
    };
    let root = OwnedRoot::open(Path::new(&path), false).unwrap();
    let name = env::var("MAESTRO_E10_PROCESS_NAME").unwrap();
    match env::var("MAESTRO_E10_PROCESS_MODE").unwrap().as_str() {
        "reader" => {
            let guard = root.open_control(ControlFile::Access).unwrap();
            guard
                .lock_with(&SystemFileLock, LockMode::Shared, false)
                .unwrap();
            let file = root.receipt_file(&name).unwrap().unwrap();
            #[cfg(feature = "engine")]
            let native = super::super::engine::cleanup_tests::reader(
                Path::new(&path),
                env::var("MAESTRO_E10_PROCESS_GENERATION")
                    .unwrap()
                    .parse()
                    .unwrap(),
            );
            say("READY");
            receive();
            #[cfg(feature = "engine")]
            drop(native);
            drop((file, guard));
        }
        "race" => {
            let guard = root.open_control(ControlFile::Access).unwrap();
            assert_eq!(
                guard
                    .lock_with(&SystemFileLock, LockMode::Shared, false)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            say("BUSY");
            receive();
            guard
                .lock_with(&SystemFileLock, LockMode::Shared, false)
                .unwrap();
            assert!(root.receipt_file(&name).unwrap().is_none());
            say("MISSING");
        }
        "crash" => {
            let data = Path::new(&path).parent().unwrap();
            let database = Database::open_in(data).unwrap();
            let scopes = database.visible("cleaner").unwrap();
            let id = env::var("MAESTRO_E10_PROCESS_JOB")
                .unwrap()
                .parse()
                .unwrap();
            let mut lease = database.job(&scopes, id).unwrap().unwrap().lease.unwrap();
            let generation = env::var("MAESTRO_E10_PROCESS_GENERATION")
                .unwrap()
                .parse()
                .unwrap();
            let cleanup =
                Cleanup::prepare(&database, "cleaner", Path::new(&path), generation, true).unwrap();
            assert_eq!(
                cleanup
                    .apply(&database, "cleaner", &mut lease, timing())
                    .unwrap(),
                CleanupOutcome::Removed
            );
            say("UNLINKED");
            receive(); // Parent kills this process before any job outcome is recorded.
        }
        other => panic!("unknown test actor {other}"),
    }
}

fn say(message: &str) {
    println!("{message}");
    io::stdout().flush().unwrap();
}

fn receive() {
    io::stdin().read_exact(&mut [0]).unwrap();
}

struct Actor {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Actor {
    fn start(fixture: &Fixture, mode: &str, job: Option<String>) -> Self {
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "graph::projection::tests::cleanup_process::cleanup_process_actor",
                "--nocapture",
            ])
            .env("MAESTRO_E10_PROCESS_ROOT", fixture.path.join("graph"))
            .env("MAESTRO_E10_PROCESS_NAME", &fixture.receipt.file_name)
            .env(
                "MAESTRO_E10_PROCESS_GENERATION",
                fixture.receipt.generation_id.to_string(),
            )
            .env("MAESTRO_E10_PROCESS_MODE", mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(job) = job {
            command.env("MAESTRO_E10_PROCESS_JOB", job);
        }
        let mut child = command.spawn().unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    }

    fn expect(&mut self, message: &str) {
        loop {
            let mut line = String::new();
            assert_ne!(
                self.output.read_line(&mut line).unwrap(),
                0,
                "actor exited before {message}"
            );
            if line.trim() == message {
                return;
            }
        }
    }

    fn finish(mut self) {
        self.input.write_all(b"x").unwrap();
        assert!(self.child.wait().unwrap().success());
    }
}

impl Drop for Actor {
    fn drop(&mut self) {
        drop(self.child.kill());
        drop(self.child.wait());
    }
}

#[test]
fn cleanup_process_live_reader_blocks_apply_and_exit_releases_guard() {
    let fixture = Fixture::new();
    fixture.retire();
    let mut actor = Actor::start(&fixture, "reader", None);
    actor.expect("READY");
    let graph = fixture.path.join("graph");
    assert_eq!(
        Cleanup::prepare(
            &fixture.database,
            "cleaner",
            &graph,
            fixture.receipt.generation_id,
            true
        )
        .unwrap_err(),
        CleanupError::Busy
    );
    assert!(
        Cleanup::prepare(
            &fixture.database,
            "cleaner",
            &graph,
            fixture.receipt.generation_id,
            false
        )
        .is_ok()
    );
    actor.finish();
    let cleanup = Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &graph,
        fixture.receipt.generation_id,
        true,
    )
    .unwrap();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut fixture.lease(), timing())
            .unwrap(),
        CleanupOutcome::Removed
    );
}

#[test]
fn cleanup_process_open_delete_race_has_no_lookup_to_unlink_gap() {
    let fixture = Fixture::new();
    fixture.retire();
    let cleanup = Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &fixture.path.join("graph"),
        fixture.receipt.generation_id,
        true,
    )
    .unwrap();
    let mut actor = Actor::start(&fixture, "race", None);
    actor.expect("BUSY");
    let mut lease = fixture.lease();
    assert_eq!(
        cleanup
            .apply(&fixture.database, "cleaner", &mut lease, timing())
            .unwrap(),
        CleanupOutcome::Removed
    );
    fixture
        .database
        .complete_job(&lease, JobState::Succeeded, &json!({"reason": "removed"}))
        .unwrap();
    drop(cleanup);
    actor.input.write_all(b"x").unwrap();
    actor.expect("MISSING");
    assert!(actor.child.wait().unwrap().success());
}

#[test]
fn cleanup_process_crash_after_unlink_retries_under_taken_over_lease() {
    let fixture = Fixture::new();
    fixture.retire();
    let lease = fixture.lease();
    let mut actor = Actor::start(&fixture, "crash", Some(lease.job.to_string()));
    actor.expect("UNLINKED");
    actor.child.kill().unwrap();
    actor.child.wait().unwrap();
    assert_eq!(
        fixture
            .database
            .job(&fixture.scopes, lease.job)
            .unwrap()
            .unwrap()
            .state,
        JobState::Running
    );
    let future = timing().now + Duration::from_secs(31);
    let mut takeover = fixture
        .database
        .take_job(lease.job, "recovery", future, timing().term)
        .unwrap();
    assert!(takeover.number > lease.number);
    let retry = Cleanup::prepare(
        &fixture.database,
        "cleaner",
        &fixture.path.join("graph"),
        fixture.receipt.generation_id,
        true,
    )
    .unwrap();
    assert_eq!(
        retry
            .apply(&fixture.database, "cleaner", &mut takeover, timing())
            .unwrap(),
        CleanupOutcome::AlreadyMissing
    );
    fixture
        .database
        .complete_job(
            &takeover,
            JobState::Succeeded,
            &json!({"reason": "already_missing"}),
        )
        .unwrap();
    assert_eq!(
        fixture
            .database
            .projection_ready(&fixture.scopes, fixture.receipt.generation_id)
            .unwrap(),
        Some(fixture.receipt.clone())
    );
}
