//! Real deaths at mandatory commit/readiness boundaries, synchronized through pipes.
#![cfg(unix)]
use super::{
    loader_tests::{fixture, snapshot, staging},
    public_fixture::{Fixture, now, settings},
};
use crate::graph::projection::{
    ProjectionBuild, ProjectionEngine, ProjectionFactory, ProjectionScope, ProjectionSnapshot,
    TypedEdgeProjection, checkpoint::prefix,
};
use maestro_filesystem::{ControlFile, OwnedRoot, SystemFileLock};
use maestro_kernel::{facts::EXACT_RESOLVER_VERSION, store::Database};
use std::{
    env, fs,
    io::{self, BufRead as _, BufReader, Write as _},
    path::PathBuf,
    process::{Child, Command, Stdio},
};

const CHILD: &str = "graph::projection::engine::loader_process_tests::loader_boundary_child";

/// Hold the private production boundary until the parent kills the process.
fn pause() {
    println!("\nLOADER:PAUSED");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut String::new()).unwrap();
    panic!("parent must kill, not release, the boundary child");
}

/// Ensure a failing parent assertion cannot leave a blocked child alive.
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        if self.0.try_wait().unwrap().is_none() {
            self.0.kill().unwrap();
            self.0.wait().unwrap();
        }
    }
}

#[test]
fn loader_boundary_child() {
    let Some(kernel_path) = env::var_os("MAESTRO_LOADER_KERNEL") else {
        return;
    };
    let kernel = Database::open_in(&PathBuf::from(kernel_path)).unwrap();
    let scopes = kernel.visible("builder").unwrap();
    let scope = ProjectionScope {
        collection_id: "graph".into(),
        generation_id: env::var("MAESTRO_LOADER_GENERATION")
            .unwrap()
            .parse()
            .unwrap(),
    };
    let lease = kernel
        .job(
            &scopes,
            env::var("MAESTRO_LOADER_JOB").unwrap().parse().unwrap(),
        )
        .unwrap()
        .unwrap()
        .lease
        .unwrap();
    let build = ProjectionBuild {
        scope: scope.clone(),
        lease,
        claim_set_id: serde_json::from_value(env::var("MAESTRO_LOADER_SET").unwrap().into())
            .unwrap(),
        resolution_id: serde_json::from_value(
            env::var("MAESTRO_LOADER_RESOLUTION").unwrap().into(),
        )
        .unwrap(),
        resolver_version: EXACT_RESOLVER_VERSION.into(),
        settings_identity: settings().identity(),
        frozen_lock: settings().frozen_lock().clone(),
    };
    let native = PathBuf::from(env::var_os("MAESTRO_LOADER_NATIVE").unwrap());
    let factory = ProjectionFactory::new(
        &native,
        ProjectionEngine::Ladybug,
        settings(),
        &SystemFileLock,
    );
    let clock = || now(0);
    let mut producer = factory
        .producer(&kernel, &scopes, build.clone(), &clock)
        .unwrap();
    let snapshot = ProjectionSnapshot::read(
        &kernel,
        &scopes,
        "builder",
        (&build.claim_set_id, &build.resolution_id),
        &scope,
    )
    .unwrap();
    if env::var("MAESTRO_LOADER_BOUNDARY").unwrap() == "commit" {
        producer.session.load_with(&snapshot, pause).unwrap();
    } else {
        producer.load(&snapshot).unwrap();
        producer
            .session
            .publish_with(&prefix(&snapshot, 2).unwrap(), pause)
            .unwrap();
    }
}

/// Kill only after the child acknowledges the production boundary.
fn kill_at_boundary(fixture: &Fixture, boundary: &str) {
    let mut process = Process(
        Command::new(env::current_exe().unwrap())
            .args(["--exact", CHILD, "--nocapture", "--test-threads=1"])
            .env("MAESTRO_LOADER_KERNEL", fixture.authority.directory())
            .env("MAESTRO_LOADER_NATIVE", &fixture.native.path)
            .env(
                "MAESTRO_LOADER_GENERATION",
                fixture.build.scope.generation_id.to_string(),
            )
            .env("MAESTRO_LOADER_JOB", fixture.build.lease.job.to_string())
            .env("MAESTRO_LOADER_SET", fixture.build.claim_set_id.as_str())
            .env(
                "MAESTRO_LOADER_RESOLUTION",
                fixture.build.resolution_id.as_str(),
            )
            .env("MAESTRO_LOADER_BOUNDARY", boundary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let mut output = BufReader::new(process.0.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert!(
            output.read_line(&mut line).unwrap() > 0,
            "child exited before {boundary} pause"
        );
        if line.trim() == "LOADER:PAUSED" {
            break;
        }
    }
    process.0.kill().unwrap();
    assert!(!process.0.wait().unwrap().success());
}

#[test]
fn loader_real_commit_death_recovers_equal_to_clean_and_ready_death_preserves_publication() {
    for boundary in ["commit", "ready"] {
        let fixture = fixture(65);
        let snapshot = snapshot(&fixture);
        kill_at_boundary(&fixture, boundary);
        let clock = || now(0);
        let path = staging(&fixture).join("loader");
        if boundary == "commit" {
            assert!(path.join("manifest.json").exists());
            assert!(!path.join("0000000001.json").exists());
            let mut resumed = fixture
                .factory()
                .resume(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    fixture.build.clone(),
                    &clock,
                )
                .unwrap();
            assert_eq!(resumed.verify().unwrap(), prefix(&snapshot, 1).unwrap());
            resumed.load(&snapshot).unwrap();
            let verified = resumed.verify().unwrap();
            let clean = super::tests::Fixture::new();
            let root = OwnedRoot::open(&clean.path, false).unwrap();
            root.ensure_control(ControlFile::Access).unwrap();
            root.ensure_control(ControlFile::Writer).unwrap();
            let clean_factory = ProjectionFactory::new(
                &clean.path,
                ProjectionEngine::Ladybug,
                settings(),
                &SystemFileLock,
            );
            let mut producer = clean_factory
                .producer(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    fixture.build.clone(),
                    &clock,
                )
                .unwrap();
            producer.load(&snapshot).unwrap();
            assert_eq!(verified, producer.verify().unwrap());
            resumed.publish(&verified).unwrap();
        } else {
            let mut names: Vec<_> = fs::read_dir(&path)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            names.sort();
            assert_eq!(
                names,
                ["0000000001.json", "0000000002.json", "manifest.json"]
            );
        }
        let reader = fixture
            .factory()
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
            .unwrap();
        assert_eq!(
            reader
                .entity_facts(
                    &fixture.authority.scopes,
                    &fixture.build.scope,
                    &snapshot.facts[0].subject
                )
                .unwrap(),
            snapshot.facts
        );
        assert!(
            fixture
                .factory()
                .resume(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    fixture.build.clone(),
                    &clock
                )
                .is_err()
        );
    }
}
