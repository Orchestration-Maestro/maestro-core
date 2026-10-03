//! Graph builds: batches recorded in order under the job's lease, each with
//! its receipt and its journal step in one write, counted once, bounded by
//! budgets a takeover does not reset, and frozen into one claim set only
//! once every batch is recorded.

use super::support::{
    ORIGINAL, Scratch, TERM, at, build_job, counts, execute, granted, label, on, plan,
    relation_claim, retries, second_revision, timing,
};
use crate::{
    artifact::Digest,
    facts::{
        Batch, BuildPlan, Claim, ClaimSetRecord, EntityKind, EntityName, Error, Object, Predicate,
        Rejection,
    },
    job::{self, Lease, PROGRESSED},
    journal::Filter,
    scope::ScopeSet,
    store::Database,
};

use std::{fs, slice};

/// The rejection of the block `block` of `revision`, for `reason`.
fn rejection(revision: &str, block: Option<&str>, reason: &str) -> Rejection {
    Rejection {
        revision_id: revision.to_owned(),
        block_id: block.map(str::to_owned),
        reason: reason.to_owned(),
    }
}

/// The batch `ordinal` of `claims` and `rejections`.
fn batch(ordinal: usize, claims: Vec<Claim>, rejections: Vec<Rejection>) -> Batch {
    Batch {
        ordinal,
        claims,
        rejections,
    }
}

/// The two batches of a build of `rev-a` then `rev-b`: `label` and
/// `retries` with one rejection, then `label` again, quoting `rev-b`.
fn two_batches() -> [Batch; 2] {
    [
        batch(
            0,
            vec![label(), retries()],
            vec![rejection("rev-a", Some("block-x"), "no type")],
        ),
        batch(1, vec![on("rev-b", label())], Vec::new()),
    ]
}

/// A database with `rev-a` and `rev-b`, and a build of both begun under a
/// lease taken at `at(0)`, with budgets of 10 claims and 10 rejections.
fn begun(scratch: &Scratch) -> (Database, BuildPlan, Lease) {
    let database = scratch.open();
    second_revision(&database);
    let plan = plan(&["rev-a", "rev-b"], 10, 10);
    let lease = build_job(&database, &plan, "worker-1");
    database
        .begin_graph_build(&ScopeSet::default_workspace(), &lease, &plan)
        .unwrap();
    (database, plan, lease)
}

/// How many steps the journal records for the job of `lease`.
fn steps(database: &Database, lease: &Lease) -> usize {
    database
        .events(
            &ScopeSet::default_workspace(),
            &Filter {
                stream: &job::stream(lease.job),
                after: 0,
                r#type: Some(PROGRESSED),
            },
        )
        .unwrap()
        .len()
}

#[test]
fn a_build_freezes_its_claim_set_only_once_every_batch_is_recorded() {
    let scratch = Scratch::new();
    let (database, plan, mut lease) = begun(&scratch);
    let all = ScopeSet::default_workspace();
    let [first, second] = two_batches();
    let receipt = database
        .record_graph_batch(&all, &mut lease, timing(1), &first)
        .unwrap();
    assert_eq!(receipt.ordinal, 0);
    assert_eq!(receipt.revision_id, "rev-a");
    assert_eq!(receipt.lease_number, 1);
    assert_eq!(receipt.claims.len(), 2);
    assert_eq!((receipt.rejected, receipt.kept), (1, 1));
    assert_eq!(lease.heartbeat, "2027-01-15T08:00:01.000Z");
    let partial = database.graph_build(&all, lease.job).unwrap().unwrap();
    assert_eq!(partial.plan, plan);
    assert_eq!(partial.batches, slice::from_ref(&receipt));
    assert_eq!(partial.claim_set_id, None);
    assert!(matches!(
        database.finish_graph_build(&all, &mut lease, timing(2)),
        Err(Error::Unfinished {
            recorded: 1,
            expected: 2
        })
    ));
    assert_eq!(counts(&scratch)[2..], [0, 0], "a partial build has no set");

    database
        .record_graph_batch(&all, &mut lease, timing(3), &second)
        .unwrap();
    let set = database
        .finish_graph_build(&all, &mut lease, timing(4))
        .unwrap();
    assert_finished(&database, &scratch, &mut lease, &set);
}

/// Completion preserves order, returns the same set on replay, and checkpoints once.
fn assert_finished(
    database: &Database,
    scratch: &Scratch,
    lease: &mut Lease,
    set: &ClaimSetRecord,
) {
    let all = ScopeSet::default_workspace();
    let claims: Vec<_> = set.claims.iter().map(|claim| claim.claim.clone()).collect();
    assert_eq!(claims, [label(), retries(), on("rev-b", label())]);
    let finished = database.graph_build(&all, lease.job).unwrap().unwrap();
    assert_eq!(finished.claim_set_id, Some(set.id.clone()));
    assert_eq!(
        finished.rejections,
        [rejection("rev-a", Some("block-x"), "no type")]
    );
    assert_eq!(steps(database, lease), 3);
    let again = database.finish_graph_build(&all, lease, timing(5)).unwrap();
    assert_eq!(&again, set);
    assert_eq!(steps(database, lease), 3, "a finished build finishes once");
    assert_eq!(counts(scratch), [3, 3, 1, 3]);
}

#[test]
fn a_batch_replayed_after_its_commit_is_not_counted_twice() {
    let scratch = Scratch::new();
    let (database, _, mut lease) = begun(&scratch);
    let all = ScopeSet::default_workspace();
    let [first, _] = two_batches();
    let receipt = database
        .record_graph_batch(&all, &mut lease, timing(1), &first)
        .unwrap();
    fs::remove_file(scratch.stored(&Digest::of(ORIGINAL.as_bytes()))).unwrap();
    let replayed = database
        .record_graph_batch(&all, &mut lease, timing(2), &first)
        .unwrap();
    assert_eq!(replayed, receipt);
    assert_eq!(steps(&database, &lease), 1);
    assert_eq!(counts(&scratch)[..2], [2, 2]);

    let mut other = first;
    other.rejections.clear();
    assert!(matches!(
        database.record_graph_batch(&all, &mut lease, timing(3), &other),
        Err(Error::Conflict(_))
    ));
    let build = database.graph_build(&all, lease.job).unwrap().unwrap();
    assert_eq!(build.batches, [receipt]);
    assert_eq!(build.rejections.len(), 1);
}

#[test]
fn a_claim_of_another_profile_or_source_is_refused_in_a_batch() {
    let scratch = Scratch::new();
    let (database, _, mut lease) = begun(&scratch);
    let all = ScopeSet::default_workspace();
    let mut changed = label();
    changed.provenance.profile = Digest::of(b"another profile");
    let refused = [
        batch(0, vec![changed], Vec::new()),
        batch(0, vec![on("rev-b", label())], Vec::new()),
        batch(0, Vec::new(), vec![rejection("rev-b", None, "elsewhere")]),
        batch(1, vec![on("rev-b", label())], Vec::new()),
        batch(2, Vec::new(), Vec::new()),
        batch(0, Vec::new(), vec![rejection("rev-a", None, "")]),
    ];
    for refused in refused {
        let error = database
            .record_graph_batch(&all, &mut lease, timing(1), &refused)
            .unwrap_err();
        assert!(matches!(error, Error::Invalid(_)), "{refused:?}: {error:?}");
    }
    assert_eq!(steps(&database, &lease), 0);
    assert_eq!(counts(&scratch), [0, 0, 0, 0]);
}

#[test]
fn an_expired_worker_writes_nothing_after_a_takeover() {
    let scratch = Scratch::new();
    let (database, _, mut stale) = begun(&scratch);
    let all = ScopeSet::default_workspace();
    let [first, second] = two_batches();
    database
        .record_graph_batch(&all, &mut stale, timing(1), &first)
        .unwrap();
    let mut current = database
        .take_job(stale.job, "worker-2", at(120), TERM)
        .unwrap();
    assert_eq!(current.number, 2);
    let late = database.record_graph_batch(&all, &mut stale, timing(121), &second);
    assert!(
        matches!(late, Err(Error::Job(job::Error::Lost { number: 1, .. }))),
        "{late:?}"
    );
    assert!(matches!(
        database.finish_graph_build(&all, &mut stale, timing(121)),
        Err(Error::Job(job::Error::Lost { .. }))
    ));
    let resumed = database.graph_build(&all, current.job).unwrap().unwrap();
    assert_eq!(
        resumed.batches.len(),
        1,
        "the takeover resumes after batch 0"
    );
    let receipt = database
        .record_graph_batch(&all, &mut current, timing(122), &second)
        .unwrap();
    assert_eq!(receipt.lease_number, 2);
    database
        .finish_graph_build(&all, &mut current, timing(123))
        .unwrap();
}

#[test]
fn budgets_count_what_earlier_leases_recorded() {
    let scratch = Scratch::new();
    let database = scratch.open();
    second_revision(&database);
    let all = ScopeSet::default_workspace();
    let plan = plan(&["rev-a", "rev-b"], 2, 1);
    let stale = build_job(&database, &plan, "worker-1");
    database.begin_graph_build(&all, &stale, &plan).unwrap();
    let mut first = stale.clone();
    let [mut zero, one] = two_batches();
    zero.rejections
        .push(rejection("rev-a", None, "a second reason"));
    let receipt = database
        .record_graph_batch(&all, &mut first, timing(1), &zero)
        .unwrap();
    assert_eq!((receipt.rejected, receipt.kept), (2, 1));
    let mut current = database
        .take_job(stale.job, "worker-2", at(120), TERM)
        .unwrap();
    let over = database.record_graph_batch(&all, &mut current, timing(121), &one);
    assert!(
        matches!(
            over,
            Err(Error::OverBudget {
                limit: 2,
                needed: 3
            })
        ),
        "{over:?}"
    );
    let rejected = batch(1, Vec::new(), vec![rejection("rev-b", None, "no table")]);
    let receipt = database
        .record_graph_batch(&all, &mut current, timing(122), &rejected)
        .unwrap();
    assert_eq!((receipt.rejected, receipt.kept), (1, 0));
    let build = database.graph_build(&all, current.job).unwrap().unwrap();
    assert_eq!(build.rejections.len(), 1);
    assert_eq!(build.rejected(), 3);
}

#[test]
fn a_build_begins_once_with_its_frozen_plan_in_its_own_scope() {
    let scratch = Scratch::new();
    let (database, plan, lease) = begun(&scratch);
    let all = ScopeSet::default_workspace();
    let again = database.begin_graph_build(&all, &lease, &plan).unwrap();
    assert_eq!(again.plan, plan);
    let mut other = plan.clone();
    other.budget.max_claims = 3;
    assert!(matches!(
        database.begin_graph_build(&all, &lease, &other),
        Err(Error::Conflict(_))
    ));
    let outsider = granted(&database, "alice", "workspace/default/collection/other");
    assert!(matches!(
        database.begin_graph_build(&outsider, &lease, &plan),
        Err(Error::Unauthorized)
    ));
    assert_eq!(database.graph_build(&outsider, lease.job).unwrap(), None);

    let foreign = super::support::plan(&["rev-o"], 1, 0);
    let lease = build_job(&database, &foreign, "worker-1");
    assert!(matches!(
        database.begin_graph_build(&all, &lease, &foreign),
        Err(Error::UnknownRevision { .. })
    ));
    for (sources, max_claims) in [
        (&[][..], 1),
        (&["rev-a", "rev-a"][..], 1),
        (&["rev-a"][..], 0),
    ] {
        let invalid = super::support::plan(sources, max_claims, 0);
        let lease = build_job(&database, &invalid, "worker-1");
        assert!(matches!(
            database.begin_graph_build(&all, &lease, &invalid),
            Err(Error::Invalid(_))
        ));
    }
}

#[test]
fn a_build_that_accepted_no_claim_has_no_set() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let all = ScopeSet::default_workspace();
    let plan = plan(&["rev-a"], 1, 1);
    let mut lease = build_job(&database, &plan, "worker-1");
    database.begin_graph_build(&all, &lease, &plan).unwrap();
    let rejected = batch(0, Vec::new(), vec![rejection("rev-a", None, "no table")]);
    database
        .record_graph_batch(&all, &mut lease, timing(1), &rejected)
        .unwrap();
    assert!(matches!(
        database.finish_graph_build(&all, &mut lease, timing(2)),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn build_receipts_never_change_nor_leave_their_order() {
    let scratch = Scratch::new();
    let (database, _, mut lease) = begun(&scratch);
    let all = ScopeSet::default_workspace();
    let [first, _] = two_batches();
    database
        .record_graph_batch(&all, &mut lease, timing(1), &first)
        .unwrap();
    for statement in [
        "DELETE FROM graph_builds",
        "UPDATE graph_builds SET max_claims = 99",
        "DELETE FROM graph_build_batches",
        "UPDATE graph_build_batches SET claim_count = 9",
        "DELETE FROM graph_build_claims",
        "UPDATE graph_build_claims SET position = 9",
        "DELETE FROM graph_build_rejections",
        "UPDATE graph_build_rejections SET reason = 'other'",
        "INSERT INTO graph_build_batches (job_id, ordinal, revision_id, lease_number,
           content_digest, claim_count, rejected, kept)
         SELECT job_id, 2, 'rev-b', 1, 'x', 0, 0, 0 FROM graph_builds",
        "INSERT INTO graph_build_batches (job_id, ordinal, revision_id, lease_number,
           content_digest, claim_count, rejected, kept)
         SELECT job_id, 1, 'rev-a', 1, 'x', 0, 0, 0 FROM graph_builds",
        "INSERT INTO graph_build_claims (job_id, ordinal, position, claim_id)
         SELECT job_id, 0, 2, claim_id FROM graph_build_claims WHERE position = 0",
        "INSERT INTO graph_build_rejections (job_id, position, ordinal, revision_id, reason)
         SELECT job_id, 5, 0, 'rev-a', 'more' FROM graph_builds",
        "UPDATE graph_builds SET claim_set_id = 'none'",
    ] {
        assert!(execute(&database, statement).is_err(), "{statement}");
    }
}

#[test]
fn entity_batches_replay_but_changed_object_names_or_kinds_conflict() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let all = ScopeSet::default_workspace();
    let plan = plan(&["rev-a"], 1, 0);
    let mut lease = build_job(&database, &plan, "worker-1");
    database.begin_graph_build(&all, &lease, &plan).unwrap();
    let claim = relation_claim(
        (EntityKind::Parameter, "p"),
        Predicate::Requires,
        (EntityKind::Component, "c"),
    );
    let first = batch(0, vec![claim.clone()], Vec::new());
    let receipt = database
        .record_graph_batch(&all, &mut lease, timing(1), &first)
        .unwrap();
    drop(database);
    let database = scratch.open();
    assert_eq!(
        database
            .record_graph_batch(&all, &mut lease, timing(2), &first)
            .unwrap(),
        receipt
    );
    for endpoint in [
        (EntityKind::Component, "different"),
        (EntityKind::Parameter, "c"),
    ] {
        let changed = relation_claim((EntityKind::Parameter, "p"), Predicate::Requires, endpoint);
        assert!(matches!(
            database.record_graph_batch(
                &all,
                &mut lease,
                timing(3),
                &batch(0, vec![changed], Vec::new())
            ),
            Err(Error::Conflict(_))
        ));
    }
    let finished = database
        .finish_graph_build(&all, &mut lease, timing(4))
        .unwrap();
    assert_eq!(finished.claims[0].claim, claim);
    assert_eq!(steps(&database, &lease), 2);
    assert_eq!(counts(&scratch), [1, 1, 1, 1]);
}

#[test]
fn batch_digest_preserves_literal_bytes_and_marks_entity_objects() {
    use crate::facts::build::batch_digest;
    use serde_json::json;

    let literal = label();
    let Object::Literal(object) = &literal.object else {
        panic!("literal fixture")
    };
    let support = &literal.supports[0];
    let claim = json!([
        literal.subject.kind.as_str(),
        literal.subject.name,
        literal.predicate.as_str(),
        object.kind.as_str(),
        object.lexeme,
        literal.conditions,
        null,
        null,
        literal.provenance.extractor,
        literal.provenance.profile.as_str(),
        [[
            support.revision_id,
            support.block_id,
            support.span.start,
            support.span.end,
            support.quote_digest.as_str()
        ]]
    ]);
    let expected = |claim| {
        Digest::of(
            json!(["maestro-graph-batch/1", 0, [claim], []])
                .to_string()
                .as_bytes(),
        )
    };
    let literal_digest = batch_digest(&batch(0, vec![literal.clone()], Vec::new()));
    assert_eq!(literal_digest, expected(claim.clone()));
    // Hold every other field constant, including the predicate: digesting precedes admission.
    let mut entity = literal.clone();
    entity.object = Object::Entity(EntityName {
        kind: EntityKind::Component,
        name: object.lexeme.clone(),
    });
    let entity_digest = batch_digest(&batch(0, vec![entity], Vec::new()));
    let mut marked = claim;
    marked[3] = json!(["entity", "Component"]);
    assert_eq!(entity_digest, expected(marked));
    assert_ne!(entity_digest, literal_digest);
}
