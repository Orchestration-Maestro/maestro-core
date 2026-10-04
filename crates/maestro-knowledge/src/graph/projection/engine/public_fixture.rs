//! Real kernel authority reused by public lifecycle tests, with private native scratch.
use super::tests::Fixture as NativeFixture;
use crate::graph::{
    descriptors::tests_source::Authority,
    projection::{
        EdgeFamily, EngineSettings, ProjectionBuild, ProjectionEdge, ProjectionEngine,
        ProjectionFactory, ProjectionScope,
    },
};
use maestro_filesystem::{ControlFile, OwnedRoot, SystemFileLock};
use maestro_kernel::facts::EXACT_RESOLVER_VERSION;
use maestro_kernel::facts::ResolutionInput;
use maestro_kernel::{artifact::Digest, facts::Object, job::NewJob, scope::collection_path};
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) struct Fixture {
    pub(super) authority: Authority,
    pub(super) native: NativeFixture,
    pub(super) build: ProjectionBuild,
    pub(super) edges: Vec<ProjectionEdge>,
}

pub(super) fn now(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000 + seconds)
}
pub(super) fn settings() -> EngineSettings {
    EngineSettings::new(
        16 * 1024 * 1024,
        64 * 1024 * 1024,
        1,
        Digest::of(b"frozen lifecycle lock"),
    )
    .unwrap()
}

impl Fixture {
    pub(super) fn new() -> Self {
        let authority = Authority::new();
        let native = NativeFixture::new();
        let scope = ProjectionScope {
            collection_id: authority.pin.collection_id.clone(),
            generation_id: authority.pin.generation_id,
        };
        let set = authority
            .database
            .graph_attachment(&authority.scopes, scope.generation_id)
            .unwrap()
            .unwrap()
            .claim_set_id;
        let claims = authority
            .database
            .claim_set(&authority.scopes, &set)
            .unwrap()
            .unwrap()
            .claims;
        let edges = claims
            .into_iter()
            .map(|record| {
                let Object::Entity(target) = &record.claim.object else {
                    panic!("entity fixture")
                };
                ProjectionEdge {
                    id: record.id,
                    scope: scope.clone(),
                    family: EdgeFamily::KnowledgeClaim,
                    source: Digest::of(record.claim.subject.name.as_bytes()),
                    target: Digest::of(target.name.as_bytes()),
                    relation: record.claim.predicate.as_str().into(),
                }
            })
            .collect();
        authority
            .database
            .verify_generation(scope.generation_id, 1)
            .unwrap();
        let body = json!({"generation": scope.generation_id});
        let kernel_scope = collection_path(&scope.collection_id).parse().unwrap();
        let job = authority
            .database
            .submit_job(
                &NewJob {
                    kind: "knowledge.graph.project",
                    inputs: &body,
                    scope: &kernel_scope,
                    resource: None,
                },
                now(0),
            )
            .unwrap();
        let lease = authority
            .database
            .take_job(job.id, "producer", now(0), Duration::from_secs(60))
            .unwrap();
        let root = OwnedRoot::open(&native.path, false).unwrap();
        root.ensure_control(ControlFile::Access).unwrap();
        root.ensure_control(ControlFile::Writer).unwrap();
        let resolution = authority
            .database
            .record_resolution(
                &authority.scopes,
                "builder",
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
        Self {
            authority,
            native,
            build: ProjectionBuild {
                build_id: 1,
                scope,
                claim_set_id: set,
                resolution_id: resolution,
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                settings_identity: settings().identity(),
                frozen_lock: settings().frozen_lock().clone(),
                lease,
            },
            edges,
        }
    }

    pub(super) fn factory(&self) -> ProjectionFactory<'_> {
        ProjectionFactory::new(
            &self.native.path,
            ProjectionEngine::Ladybug,
            settings(),
            &SystemFileLock,
        )
    }
}
