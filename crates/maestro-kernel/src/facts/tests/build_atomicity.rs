//! Transaction boundaries, restart receipts, concurrent replay and frozen job identity.

use super::support::{Scratch, at, build_job, counts, execute, label, plan, submit_build, timing};
use crate::{
    artifact::Digest,
    facts::{Batch, Error, Rejection},
    job::{self, Lease, LeaseTiming},
    scope::ScopeSet,
    store::Database,
};
use std::{
    sync::Barrier,
    thread,
    time::{Duration, UNIX_EPOCH},
};

/// A one-source build ready to record one claim and two retained rejections.
fn ready(scratch: &Scratch) -> (Database, Lease, Batch) {
    let database = scratch.open();
    let plan = plan(&["rev-a"], 1, 2);
    let lease = build_job(&database, &plan, "worker");
    database
        .begin_graph_build(&ScopeSet::default_workspace(), &lease, &plan)
        .unwrap();
    let rejections = ["untyped", "unlocated"]
        .map(|reason| Rejection {
            revision_id: "rev-a".into(),
            block_id: None,
            reason: reason.into(),
        })
        .to_vec();
    (
        database,
        lease,
        Batch {
            ordinal: 0,
            claims: vec![label()],
            rejections,
        },
    )
}

#[test]
fn a_checkpoint_failure_rolls_back_claims_receipts_and_renewal() {
    let scratch = Scratch::new();
    let (database, mut lease, batch) = ready(&scratch);
    let before = lease.clone();
    let all = ScopeSet::default_workspace();
    execute(
        &database,
        "CREATE TRIGGER refuse_checkpoint BEFORE INSERT ON events
        BEGIN SELECT RAISE(ABORT, 'injected checkpoint failure'); END",
    )
    .unwrap();
    assert!(
        database
            .record_graph_batch(&all, &mut lease, timing(1), &batch)
            .is_err()
    );
    assert_eq!(lease, before);
    assert_eq!(
        database.job(&all, lease.job).unwrap().unwrap().lease,
        Some(before)
    );
    assert_eq!(counts(&scratch), [0, 0, 0, 0]);
    let build = database.graph_build(&all, lease.job).unwrap().unwrap();
    assert!(build.batches.is_empty());
    assert!(build.rejections.is_empty());
    execute(&database, "DROP TRIGGER refuse_checkpoint").unwrap();
    database
        .record_graph_batch(&all, &mut lease, timing(2), &batch)
        .unwrap();
}

#[test]
fn invalid_renewal_times_cannot_commit_a_batch_or_finish() {
    let scratch = Scratch::new();
    let (database, mut lease, batch) = ready(&scratch);
    let all = ScopeSet::default_workspace();
    let invalid = LeaseTiming {
        now: UNIX_EPOCH - Duration::from_secs(1),
        term: Duration::ZERO,
    };
    let before = lease.clone();
    assert!(matches!(
        database.record_graph_batch(&all, &mut lease, invalid, &batch),
        Err(Error::Job(job::Error::Time))
    ));
    assert_eq!(lease, before);
    assert_eq!(counts(&scratch), [0, 0, 0, 0]);
    database
        .record_graph_batch(&all, &mut lease, timing(1), &batch)
        .unwrap();
    let before = lease.clone();
    assert!(matches!(
        database.finish_graph_build(&all, &mut lease, invalid),
        Err(Error::Job(job::Error::Time))
    ));
    assert_eq!(lease, before);
    assert_eq!(counts(&scratch), [1, 1, 0, 0]);
    assert_eq!(
        database
            .graph_build(&all, lease.job)
            .unwrap()
            .unwrap()
            .claim_set_id,
        None
    );
    database
        .finish_graph_build(&all, &mut lease, timing(2))
        .unwrap();
}

#[test]
fn malformed_collection_and_overflowed_budgets_are_refused_before_writes() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut plan = plan(&["rev-a"], 1, 0);
    let lease = build_job(&database, &plan, "worker");
    let all = ScopeSet::default_workspace();
    for collection in ["", "graph/source/docs", "graph/", "graph-g9"] {
        plan.collection_id = collection.into();
        assert!(matches!(
            database.begin_graph_build(&all, &lease, &plan),
            Err(Error::Unauthorized)
        ));
    }
    plan.collection_id = "graph".into();
    plan.budget.max_claims = usize::MAX;
    assert!(matches!(
        database.begin_graph_build(&all, &lease, &plan),
        Err(Error::Invalid(_))
    ));
    assert_eq!(database.graph_build(&all, lease.job).unwrap(), None);
    assert_eq!(counts(&scratch), [0, 0, 0, 0]);
}

#[test]
fn racing_identical_batches_commit_one_receipt_then_reopen_and_resume() {
    let scratch = Scratch::new();
    let (database, lease, batch) = ready(&scratch);
    let barrier = Barrier::new(2);
    let receipts = thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    let connection = Database::open_in(&scratch.0).unwrap();
                    let mut worker = lease.clone();
                    barrier.wait();
                    connection
                        .record_graph_batch(
                            &ScopeSet::default_workspace(),
                            &mut worker,
                            timing(1),
                            &batch,
                        )
                        .unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(receipts.first(), receipts.last());
    drop(database);
    let database = Database::open_in(&scratch.0).unwrap();
    let all = ScopeSet::default_workspace();
    let mut resumed = database
        .take_job(lease.job, "resumed", at(120), Duration::from_mins(1))
        .unwrap();
    let build = database.graph_build(&all, lease.job).unwrap().unwrap();
    assert_eq!(
        build.batches,
        receipts.first().cloned().into_iter().collect::<Vec<_>>()
    );
    assert_eq!(build.rejections, batch.rejections);
    let replay = database
        .record_graph_batch(&all, &mut resumed, timing(121), &batch)
        .unwrap();
    assert_eq!(Some(&replay), receipts.first());
    assert_eq!(replay.lease_number, 1);
    let set = database
        .finish_graph_build(&all, &mut resumed, timing(122))
        .unwrap();
    assert_eq!(set.claims, replay.claims);
    assert_eq!(counts(&scratch), [1, 1, 1, 1]);
}

#[test]
fn the_same_frozen_plan_deduplicates_its_job() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let plan = plan(&["rev-a"], 1, 2);
    let first = submit_build(&database, &plan);
    assert_eq!(submit_build(&database, &plan).id, first.id);
}

#[test]
fn a_different_extractor_or_profile_is_a_different_job() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let original = plan(&["rev-a"], 1, 2);
    let first = submit_build(&database, &original);
    let mut changed = original.clone();
    changed.provenance.extractor = "another-extractor/1".into();
    assert_ne!(submit_build(&database, &changed).id, first.id);
    changed = original;
    changed.provenance.profile = Digest::of(b"another profile");
    assert_ne!(submit_build(&database, &changed).id, first.id);
}
