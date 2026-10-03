//! Exact caller-clock project expiry, fencing, scope and renewal checks.
use super::{
    projection::{attached, projection_lease},
    support::{granted, timing},
};
use crate::job::{self, JobState};
use serde_json::json;

#[test]
fn projection_readiness_expiry_without_takeover_refuses_and_valid_neighbor_succeeds() {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    // Expiry does not require a successor to take the lease first.
    for now in [timing(64).now, timing(65).now] {
        assert!(
            database
                .record_projection_ready(&all, &receipt, &lease, now)
                .is_err()
        );
    }
    assert_eq!(
        database
            .projection_ready(&all, receipt.generation_id)
            .unwrap(),
        None
    );
    database
        .record_projection_ready(&all, &receipt, &lease, timing(63).now)
        .unwrap();
}

#[test]
fn projection_lease_preflight_checks_scope_kind_generation_cancellation_and_authoritative_renewal()
{
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.generation_id);
    let target = (receipt.collection_id.as_str(), receipt.generation_id);
    database
        .validate_projection_lease(&all, target, &lease, timing(5).now)
        .unwrap();
    for wrong in [
        ("other", receipt.generation_id),
        ("graph", receipt.generation_id + 1),
        ("graph", 0),
    ] {
        assert!(
            database
                .validate_projection_lease(&all, wrong, &lease, timing(5).now)
                .is_err()
        );
    }
    let denied = granted(
        &database,
        "preflight-denied",
        "workspace/default/collection/other",
    );
    assert!(
        database
            .validate_projection_lease(&denied, target, &lease, timing(5).now)
            .is_err()
    );
    let scope = "workspace/default/collection/graph".parse().unwrap();
    let inputs = json!({"generation":receipt.generation_id});
    let wrong_job = database
        .submit_job(
            &job::NewJob {
                kind: "knowledge.graph.other",
                inputs: &inputs,
                scope: &scope,
                resource: None,
            },
            timing(5).now,
        )
        .unwrap();
    let wrong_lease = database
        .take_job(wrong_job.id, "wrong", timing(5).now, timing(5).term)
        .unwrap();
    assert!(
        database
            .validate_projection_lease(&all, target, &wrong_lease, timing(5).now)
            .is_err()
    );
    database
        .complete_job(&lease, JobState::Cancelled, &json!({}))
        .unwrap();
    assert!(
        database
            .validate_projection_publication(&all, &receipt, &lease, timing(5).now)
            .is_err()
    );
    let lease = projection_lease(&database, receipt.generation_id);
    let mut renewed = lease.clone();
    database
        .heartbeat(&mut renewed, timing(63).now, timing(63).term)
        .unwrap();
    // A stale copy of the lease cannot make authoritative renewal look expired.
    database
        .record_projection_ready(&all, &receipt, &lease, timing(70).now)
        .unwrap();
}
