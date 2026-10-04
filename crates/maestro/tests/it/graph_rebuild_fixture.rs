//! Public kernel build/attachment and real admitted-lock inputs for CLI rebuild.
#![cfg(all(feature = "engine", not(windows)))]
use super::{
    graph_build::{COLLECTION, exited, pilot, text},
    graph_operations::admitted_workspace,
    support::{Ended, Home},
};
use maestro_filesystem::SystemFileLock;
use maestro_kernel::{
    artifact::Digest,
    chunk_set::NewChunkSet,
    facts::{EXACT_RESOLVER_VERSION, ResolutionInput},
    generation::NewGeneration,
    job::{Lease, NewJob},
    scope::{LOCAL, collection_path},
};
use maestro_knowledge::graph::projection::{
    EngineSettings, ProjectionBuild, ProjectionEngine, ProjectionFactory, ProjectionScope,
    ProjectionSnapshot,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};
use ulid::Ulid;

pub(super) struct Fixture {
    pub(super) home: Home,
    pub(super) workspace: PathBuf,
    pub(super) generation: i64,
    pub(super) set: Digest,
    pub(super) resolution: Digest,
    pub(super) settings: EngineSettings,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let home = Home::new();
        let rule = pilot(&home);
        let built = home.run(&[
            "--json",
            "knowledge",
            "graph",
            "build",
            "--collection",
            COLLECTION,
            "--rule",
            text(&rule),
        ]);
        exited(&built, 0);
        let build: Ulid = built.json()["job"].as_str().unwrap().parse().unwrap();
        let database = home.database();
        database
            .begin_chunk_set(&NewChunkSet {
                id: "rebuild-chunks",
                collection_id: COLLECTION,
                chunk_profile: "test",
                counter_contract_id: "test",
            })
            .unwrap();
        let generation = database
            .create_generation(&NewGeneration {
                collection_id: COLLECTION.into(),
                chunk_set_id: "rebuild-chunks".into(),
                embedding_profile: "test".into(),
                sparse_profile: "test".into(),
            })
            .unwrap()
            .id;
        exited(
            &home.run(&[
                "knowledge",
                "graph",
                "attach",
                "--build",
                &build.to_string(),
                "--generation",
                &generation.to_string(),
            ]),
            0,
        );
        database.verify_generation(generation, 0).unwrap();
        let scopes = database.visible(LOCAL).unwrap();
        let set = database
            .graph_attachment(&scopes, generation)
            .unwrap()
            .unwrap()
            .claim_set_id;
        let resolution = database
            .record_resolution(
                &scopes,
                LOCAL,
                &ResolutionInput {
                    resolver_version: EXACT_RESOLVER_VERSION.into(),
                    sets: vec![set.clone()],
                    previous: None,
                    decisions: vec![],
                },
                &|_| Ok(()),
            )
            .unwrap()
            .id;
        let workspace = admitted_workspace(
            &home,
            concat!(
                "schema = 'maestro-preferences/1'\n[overrides]\n",
                "'graphdb.buffer_pool_size' = 16777216\n'graphdb.max_db_size' = 67108864\n",
                "'graphdb.max_num_threads' = 1\n"
            ),
        );
        let lock = fs::read(workspace.join(".maestro/authoring.lock.json")).unwrap();
        let settings =
            EngineSettings::new(16 * 1024 * 1024, 64 * 1024 * 1024, 1, Digest::of(&lock)).unwrap();
        super::graph_cleanup_support::guards(&home);
        Self {
            home,
            workspace,
            generation,
            set,
            resolution,
            settings,
        }
    }
    pub(super) fn run(&self, extra: &[&str]) -> Ended {
        let generation = self.generation.to_string();
        let args = [
            &[
                "--set",
                "graph.engine=ladybug",
                "--json",
                "knowledge",
                "graph",
                "rebuild",
                "--generation",
                &generation,
            ][..],
            extra,
        ]
        .concat();
        self.home.run_in(&self.workspace, &args)
    }
    pub(super) fn scope(&self) -> ProjectionScope {
        ProjectionScope {
            collection_id: COLLECTION.into(),
            generation_id: self.generation,
        }
    }
    pub(super) fn factory(&self) -> ProjectionFactory<'static> {
        ProjectionFactory::new(
            &self.home.data().join("graph"),
            ProjectionEngine::Ladybug,
            self.settings.clone(),
            &SystemFileLock,
        )
    }
    pub(super) fn interrupted(&self) -> (Ulid, PathBuf) {
        let database = self.home.database();
        let scopes = database.visible(LOCAL).unwrap();
        let now = SystemTime::UNIX_EPOCH;
        let scope = collection_path(COLLECTION).parse().unwrap();
        let job = database
            .submit_job(
                &NewJob {
                    kind: "knowledge.graph.project",
                    inputs: &json!({"generation":self.generation}),
                    scope: &scope,
                    resource: Some(&format!("graph-project:{}", self.generation)),
                },
                now,
            )
            .unwrap();
        let lease = database
            .take_job(job.id, "interrupted", now, Duration::from_secs(60))
            .unwrap();
        let build = self.build(lease.clone());
        let snapshot = ProjectionSnapshot::read(
            &database,
            &scopes,
            LOCAL,
            (&self.set, &self.resolution),
            &self.scope(),
        )
        .unwrap();
        let clock = || now;
        let factory = self.factory();
        let mut producer = factory.producer(&database, &scopes, build, &clock).unwrap();
        producer.load(&snapshot).unwrap();
        drop(producer);
        (
            job.id,
            self.home
                .data()
                .join("graph")
                .join(format!(".build-{}", job.id)),
        )
    }
    pub(super) fn build(&self, lease: Lease) -> ProjectionBuild {
        ProjectionBuild {
            scope: self.scope(),
            claim_set_id: self.set.clone(),
            resolution_id: self.resolution.clone(),
            resolver_version: EXACT_RESOLVER_VERSION.into(),
            settings_identity: self.settings.identity(),
            frozen_lock: self.settings.frozen_lock().clone(),
            lease,
        }
    }
}
