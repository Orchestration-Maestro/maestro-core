//! Pipes acknowledge guard ownership; no sleeps or PID-file liveness guesses.
#![cfg(test)]
use super::fixture::{Fixture, factory};
use maestro_filesystem::{ControlFile, LockMode, OwnedRoot, SystemFileLock};
use maestro_kernel::{artifact::Digest, job::LeaseTiming, store::Database};
use maestro_knowledge::graph::projection::{
    BuildVerification, EntityFact, ProjectionBuild, ProjectionScope, TypedEdgeProjection,
};
use std::{
    env, fs,
    io::{self, BufRead as _, BufReader, Write as _},
    path::PathBuf,
    process::{Child, ChildStdout, Command, Stdio},
    slice,
    time::{Duration, SystemTime},
};

const CHILD_TEST: &str = "projection_lifecycle::native_processes::\
    lifecycle_process_child_owns_native_handle";

struct Process {
    child: Child,
    output: BufReader<ChildStdout>,
}
impl Process {
    fn spawn(fixture: &Fixture, mode: &str) -> Self {
        let mut child = Command::new(env::current_exe().unwrap())
            .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
            .env("MAESTRO_LIFECYCLE_ROOT", fixture.path())
            .env("MAESTRO_LIFECYCLE_MODE", mode)
            .env(
                "MAESTRO_LIFECYCLE_NOW",
                fixture
                    .now
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_string(),
            )
            .env(
                "MAESTRO_LIFECYCLE_GENERATION",
                fixture.build.scope.generation_id.to_string(),
            )
            .env("MAESTRO_LIFECYCLE_JOB", fixture.build.lease.job.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self { child, output }
    }
    fn signal(&mut self) -> String {
        loop {
            let mut line = String::new();
            assert!(
                self.output.read_line(&mut line).unwrap() > 0,
                "child ended before acknowledgement: {:?}",
                self.child.wait()
            );
            if let Some(signal) = line.trim().strip_prefix("LIFECYCLE:") {
                return signal.to_owned();
            }
        }
    }
    fn release(&mut self, command: &str) {
        writeln!(self.child.stdin.as_mut().unwrap(), "{command}").unwrap();
        self.child.stdin.as_mut().unwrap().flush().unwrap();
    }
    fn success(&mut self) {
        assert!(self.child.wait().unwrap().success());
    }
    fn kill(&mut self) {
        self.child.kill().unwrap();
        assert!(!self.child.wait().unwrap().success());
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            self.child.kill().unwrap();
            self.child.wait().unwrap();
        }
    }
}

fn acknowledge(signal: &str) {
    println!("\nLIFECYCLE:{signal}");
    io::stdout().flush().unwrap();
}

#[test]
fn lifecycle_process_child_owns_native_handle() {
    let Some(path) = env::var_os("MAESTRO_LIFECYCLE_ROOT") else {
        return;
    };
    let path = PathBuf::from(path);
    let kernel = Database::open_in(&path).unwrap();
    let scopes = kernel.visible("lifecycle").unwrap();
    let scope = ProjectionScope {
        collection_id: "graph".into(),
        generation_id: env::var("MAESTRO_LIFECYCLE_GENERATION")
            .unwrap()
            .parse()
            .unwrap(),
    };
    let mode = env::var("MAESTRO_LIFECYCLE_MODE").unwrap();
    let factory = factory(&path.join("graph"));
    if mode == "reader" {
        let reader = factory.reader(&kernel, &scopes, scope.clone()).unwrap();
        assert_eq!(
            reader
                .entity_facts(&scopes, &scope, &Digest::of(b"mode"))
                .unwrap()
                .len(),
            1
        );
        acknowledge("HELD");
        io::stdin().read_line(&mut String::new()).unwrap();
        drop(reader);
        return;
    }
    if mode == "not_ready" {
        assert!(factory.reader(&kernel, &scopes, scope).is_err());
        acknowledge("REFUSED");
        return;
    }
    let job_id = env::var("MAESTRO_LIFECYCLE_JOB").unwrap().parse().unwrap();
    let lease = kernel.job(&scopes, job_id).unwrap().unwrap().lease.unwrap();
    let set = kernel
        .graph_attachment(&scopes, scope.generation_id)
        .unwrap()
        .unwrap()
        .claim_set_id;
    let build = ProjectionBuild {
        scope: scope.clone(),
        claim_set_id: set.clone(),
        lease,
    };
    let now = SystemTime::UNIX_EPOCH
        + Duration::from_secs(env::var("MAESTRO_LIFECYCLE_NOW").unwrap().parse().unwrap());
    let clock = || now;
    let producer = factory.producer(&kernel, &scopes, build, &clock);
    let Ok(mut producer) = producer else {
        acknowledge("REFUSED");
        return;
    };
    let claim = kernel
        .claim_set(&scopes, &set)
        .unwrap()
        .unwrap()
        .claims
        .remove(0);
    let fact = EntityFact {
        claim,
        subject: Digest::of(b"mode"),
        scope,
    };
    let expected = BuildVerification::expected(&[], slice::from_ref(&fact)).unwrap();
    producer.write_batch(&[], &[fact]).unwrap();
    acknowledge("HELD");
    let mut command = String::new();
    io::stdin().read_line(&mut command).unwrap();
    if command.trim() == "publish" {
        assert_eq!(producer.verify().unwrap(), expected);
        producer.publish(&expected).unwrap();
        acknowledge("PUBLISHED");
    }
}

#[test]
fn lifecycle_process_writer_reader_contention_second_writer_and_valid_reopen() {
    let fixture = Fixture::new();
    let root = OwnedRoot::open(&fixture.graph(), false).unwrap();
    let cleanup = root.open_control(ControlFile::Access).unwrap();
    let mut writer = Process::spawn(&fixture, "writer");
    assert_eq!(writer.signal(), "HELD");
    assert!(
        cleanup
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .is_err()
    );
    let mut too_early = Process::spawn(&fixture, "not_ready");
    assert_eq!(too_early.signal(), "REFUSED");
    too_early.success();
    let mut second_writer = Process::spawn(&fixture, "writer");
    assert_eq!(second_writer.signal(), "REFUSED");
    second_writer.success();
    writer.release("publish");
    assert_eq!(writer.signal(), "PUBLISHED");
    writer.success();
    let mut first = Process::spawn(&fixture, "reader");
    let mut second = Process::spawn(&fixture, "reader");
    assert_eq!(first.signal(), "HELD");
    assert_eq!(second.signal(), "HELD");
    assert!(
        cleanup
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .is_err()
    );
    first.release("exit");
    first.success();
    assert!(
        cleanup
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .is_err()
    );
    second.release("exit");
    second.success();
    cleanup
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    drop(cleanup);
    let mut reopened = Process::spawn(&fixture, "reader");
    assert_eq!(reopened.signal(), "HELD");
    reopened.release("exit");
    reopened.success();
}

#[test]
fn lifecycle_process_death_preserves_staging_then_new_lease_can_publish_and_read() {
    let mut fixture = Fixture::new();
    let mut writer = Process::spawn(&fixture, "writer");
    assert_eq!(writer.signal(), "HELD");
    writer.kill();
    let orphans: Vec<_> = fs::read_dir(fixture.graph())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    assert_eq!(orphans.len(), 1);
    fixture.now += Duration::from_secs(60);
    let timing = LeaseTiming {
        now: fixture.now,
        term: Duration::from_secs(60),
    };
    fixture.build.lease = fixture
        .kernel
        .take_job(
            fixture.build.lease.job,
            "successor",
            timing.now,
            timing.term,
        )
        .unwrap();
    let mut successor = Process::spawn(&fixture, "writer");
    assert_eq!(successor.signal(), "HELD");
    successor.release("publish");
    assert_eq!(successor.signal(), "PUBLISHED");
    successor.success();
    assert!(orphans[0].is_dir());
    let mut reader = Process::spawn(&fixture, "reader");
    assert_eq!(reader.signal(), "HELD");
    reader.kill();
    let root = OwnedRoot::open(&fixture.graph(), false).unwrap();
    root.open_control(ControlFile::Access)
        .unwrap()
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    let mut reopened = Process::spawn(&fixture, "reader");
    assert_eq!(reopened.signal(), "HELD");
    reopened.release("exit");
    reopened.success();
}
