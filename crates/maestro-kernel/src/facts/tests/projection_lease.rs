//! Exact caller-clock project expiry, fencing, scope and renewal checks.
use super::{
    projection::{attached, projection_lease},
    support::{granted, timing},
};
use crate::{
    facts::{Error, ProjectionReceipt},
    job::{self, JobState, Lease},
    journal::Filter,
    scope::ScopeSet,
    store::Database,
};
use serde_json::json;

/// The three projection gates and completion must use the same credential fence.
type Check = fn(&Database, &ScopeSet, &ProjectionReceipt, &Lease) -> Result<(), Error>;

#[test]
fn projection_holder_mismatch_refuses_without_writes() {
    let operations: [(&str, Check); 3] = [
        ("lease", |database, scopes, receipt, lease| {
            database.validate_projection_lease(
                scopes,
                (
                    &receipt.identity.collection_id,
                    receipt.identity.generation_id,
                ),
                lease,
                timing(5).now,
            )
        }),
        ("publication", |database, scopes, receipt, lease| {
            database.validate_projection_publication(scopes, receipt, lease, timing(5).now)
        }),
        ("readiness", |database, scopes, receipt, lease| {
            database.record_projection_ready(scopes, receipt, lease, timing(5).now)
        }),
    ];
    for (name, check) in operations {
        holder_mismatch_refuses(name, check);
    }
}

#[test]
fn completion_holder_mismatch_refuses_without_writes() {
    holder_mismatch_refuses("completion", |database, _scopes, _receipt, lease| {
        database
            .complete_job(lease, JobState::Succeeded, &json!({}))
            .map(|_| ())
            .map_err(Error::from)
    });
}

/// Each operation gets a fresh authority; a refusal preserves its exact job/journal/readiness.
fn holder_mismatch_refuses(name: &str, check: Check) {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    let snapshot = || {
        (
            database.job(&all, lease.job).unwrap(),
            database
                .events(
                    &all,
                    &Filter {
                        stream: &job::stream(lease.job),
                        after: 0,
                        r#type: None,
                    },
                )
                .unwrap(),
            database
                .projection_ready(&all, receipt.identity.generation_id)
                .unwrap(),
        )
    };
    let before = snapshot();
    let mut wrong = lease.clone();
    wrong.holder = "wrong-holder".to_owned();
    let error = check(&database, &all, &receipt, &wrong).unwrap_err();
    let Error::Job(job::Error::Lost {
        job,
        holder,
        number,
    }) = error
    else {
        panic!("{name}: {error}");
    };
    assert_eq!(
        (job, holder.as_str(), number),
        (lease.job, "wrong-holder", lease.number)
    );
    assert_eq!(
        snapshot(),
        before,
        "{name} changed job, journal or readiness"
    );
    check(&database, &all, &receipt, &lease).unwrap();
}

#[test]
fn projection_current_holder_can_cancel_after_expiry_without_takeover() {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    assert!(
        database
            .validate_projection_lease(
                &all,
                (
                    &receipt.identity.collection_id,
                    receipt.identity.generation_id
                ),
                &lease,
                timing(65).now
            )
            .is_err()
    );
    let ended = database
        .complete_job(&lease, JobState::Cancelled, &json!({}))
        .unwrap();
    assert_eq!(ended.state, JobState::Cancelled);
    assert_eq!(
        database
            .projection_ready(&all, receipt.identity.generation_id)
            .unwrap(),
        None
    );
}

#[test]
fn projection_readiness_expiry_without_takeover_refuses_and_valid_neighbor_succeeds() {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
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
            .projection_ready(&all, receipt.identity.generation_id)
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
    let (_scratch, database, all, mut receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    let target = (
        receipt.identity.collection_id.as_str(),
        receipt.identity.generation_id,
    );
    database
        .validate_projection_lease(&all, target, &lease, timing(5).now)
        .unwrap();
    for wrong in [
        ("other", receipt.identity.generation_id),
        ("graph", receipt.identity.generation_id + 1),
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
    let inputs = json!({"generation":receipt.identity.generation_id});
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
    let lease = projection_lease(&database, receipt.identity.generation_id);
    receipt.identity.build_id = database
        .projection_build_for_job(&all, lease.job)
        .unwrap()
        .unwrap()
        .build_id;
    let mut renewed = lease.clone();
    database
        .heartbeat(&mut renewed, timing(63).now, timing(63).term)
        .unwrap();
    // A stale copy of the lease cannot make authoritative renewal look expired.
    database
        .record_projection_ready(&all, &receipt, &lease, timing(70).now)
        .unwrap();
}
