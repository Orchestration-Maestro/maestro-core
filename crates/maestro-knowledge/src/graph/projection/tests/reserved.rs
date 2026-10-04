//! Private fixtures reserve through canonical typed admission, never raw build inserts.
#[cfg(all(feature = "engine", unix))]
use crate::graph::projection::ProjectionBuild;
use maestro_kernel::{
    facts::ProjectionBuildRequest,
    job::{Lease, NewJob},
    scope::{ScopeSet, collection_path},
    store::Database,
};
use std::time::{Duration, SystemTime};

#[cfg(all(feature = "engine", unix))]
pub(in crate::graph::projection) fn reserve(
    authority: (&Database, &ScopeSet),
    build: &mut ProjectionBuild,
    predecessor: Option<i64>,
    now: SystemTime,
) {
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
    (build.build_id, build.lease) = reserve_request(authority, &request, now);
}

pub(super) fn reserve_request(
    authority: (&Database, &ScopeSet),
    request: &ProjectionBuildRequest,
    now: SystemTime,
) -> (i64, Lease) {
    let (database, scopes) = authority;
    let inputs = request.inputs();
    let scope = collection_path(&request.collection_id).parse().unwrap();
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
    let lease = database
        .take_job(job.id, "reserved-fixture", now, Duration::from_secs(60))
        .unwrap();
    let build_id = database
        .begin_projection_build(scopes, request, &lease, now)
        .unwrap()
        .build_id;
    (build_id, lease)
}
