//! Shared cutover fixtures: apply unregistered 0031 once, then reserve through
//! typed admission. Step 4 callers must use these instead of raw build inserts.
use crate::graph::projection::ProjectionBuild;
use maestro_kernel::{
    facts::ProjectionBuildRequest,
    job::NewJob,
    scope::{ScopeSet, collection_path},
    store::Database,
};
use rusqlite::Connection;
use std::{
    path::Path,
    time::{Duration, SystemTime},
};

pub(in crate::graph::projection) fn migrate(directory: &Path) {
    Connection::open(directory.join("kernel.sqlite3"))
        .unwrap()
        .execute_batch(include_str!(
            "../../../../../maestro-kernel/migrations/0031_graph_projection_builds.sql"
        ))
        .unwrap();
}

pub(in crate::graph::projection) fn reserve(
    authority: (&Database, &ScopeSet),
    build: &mut ProjectionBuild,
    predecessor: Option<i64>,
    now: SystemTime,
) {
    let (database, scopes) = authority;
    let request = ProjectionBuildRequest {
        collection_id: build.scope.collection_id.clone(),
        generation_id: build.scope.generation_id,
        claim_set_id: build.claim_set_id.clone(),
        resolution_id: build.resolution_id.clone(),
        resolver_version: build.resolver_version.clone(),
        settings_identity: build.settings_identity.clone(),
        frozen_lock: build.frozen_lock.clone(),
        expected_active_build_id: predecessor,
    };
    let inputs = request.inputs();
    let scope = collection_path(&build.scope.collection_id).parse().unwrap();
    let job = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.project",
                inputs: &inputs,
                scope: &scope,
                resource: None,
            },
            now,
        )
        .unwrap();
    build.lease = database
        .take_job(job.id, "reserved-fixture", now, Duration::from_secs(60))
        .unwrap();
    build.build_id = database
        .begin_projection_build(scopes, &request, &build.lease, now)
        .unwrap()
        .build_id;
}
