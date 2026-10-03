//! Build identities and supersession endpoint contracts.

use super::support::{Scratch, build_job, granted, label, plan, relation_claim, set_of, timing};
use crate::{
    facts::{
        Batch, Decision, DecisionKind, Endpoint, EntityKind, Error, Mention, Predicate,
        ResolutionInput, Validity,
    },
    scope::{ScopeSet, WORKSPACE},
};

#[test]
fn build_plan_refuses_an_empty_extractor_before_writing() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut plan = plan(&["rev-a"], 1, 0);
    plan.provenance.extractor.clear();
    let lease = build_job(&database, &plan, "worker");
    assert!(matches!(
        database.begin_graph_build(&ScopeSet::default_workspace(), &lease, &plan),
        Err(Error::Invalid(message)) if message == "invalid graph build plan"
    ));
    assert_eq!(
        database
            .graph_build(&ScopeSet::default_workspace(), lease.job)
            .unwrap(),
        None
    );
}

#[test]
fn batch_replay_conflicts_when_only_bounded_validity_changes() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let all = ScopeSet::default_workspace();
    let plan = plan(&["rev-a"], 1, 0);
    let mut lease = build_job(&database, &plan, "worker");
    database.begin_graph_build(&all, &lease, &plan).unwrap();
    let mut batch = Batch {
        ordinal: 0,
        claims: vec![label()],
        rejections: vec![],
    };
    batch.claims[0].version = Validity::Bounded {
        start: Some("1".into()),
        end: Some("3".into()),
    };
    batch.claims[0].world = Validity::Bounded {
        start: None,
        end: Some("2027".into()),
    };
    let receipt = database
        .record_graph_batch(&all, &mut lease, timing(1), &batch)
        .unwrap();
    assert_eq!(
        database
            .record_graph_batch(&all, &mut lease, timing(2), &batch)
            .unwrap(),
        receipt
    );
    for world in [false, true] {
        let mut changed = batch.clone();
        let validity = if world {
            &mut changed.claims[0].world
        } else {
            &mut changed.claims[0].version
        };
        *validity = Validity::Bounded {
            start: Some("2".into()),
            end: None,
        };
        assert!(matches!(
            database.record_graph_batch(&all, &mut lease, timing(3), &changed),
            Err(Error::Conflict(_))
        ));
    }
}

#[test]
fn supersession_requires_distinct_subjects_even_with_entity_objects() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = granted(&database, "reviewer", WORKSPACE);
    let claims = ["left", "right"].map(|name| {
        relation_claim(
            (EntityKind::Parameter, name),
            Predicate::Requires,
            (EntityKind::Component, "object"),
        )
    });
    let set = database
        .record_claim_set(&scopes, &set_of(claims.to_vec()))
        .unwrap();
    for (left, right, same) in [
        (Endpoint::Object, Endpoint::Subject, false),
        (Endpoint::Subject, Endpoint::Object, false),
        (Endpoint::Subject, Endpoint::Subject, true),
    ] {
        let decision = Decision {
            left: Mention {
                claim: set.claims[0].id.clone(),
                endpoint: left,
            },
            right: Mention {
                claim: set.claims[usize::from(!same)].id.clone(),
                endpoint: right,
            },
            kind: DecisionKind::Supersedes,
            reason: "reviewed replacement".into(),
        };
        let input = ResolutionInput {
            resolver_version: "exact-test/1".into(),
            sets: vec![set.id.clone()],
            previous: None,
            decisions: vec![decision],
        };
        let result = database.record_resolution(&scopes, "reviewer", &input, &|_| Ok(()));
        let expected = "supersession needs two distinct claim subjects";
        assert!(matches!(result, Err(Error::Invalid(message)) if message == expected));
    }
}
