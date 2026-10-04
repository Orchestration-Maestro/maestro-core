//! Migrated identities and concurrent submissions keep one build/job authority.
use super::{
    projection_reservation::{lease, request},
    projection_schema_support::Fixture,
    support::timing,
};
use crate::{
    facts::{Error, projection_reservation as authority},
    job::NewJob,
    scope::{Scope, ScopeSet},
};
use std::thread;

#[test]
fn projection_reservation_identical_concurrent_requests_deduplicate() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let request = request(&fixture, None);
    let scope: Scope = "workspace/default/collection/graph".parse().unwrap();
    let inputs = request.inputs();
    let submit = || {
        fixture
            .database
            .submit_job(
                &NewJob {
                    kind: "knowledge.graph.project",
                    inputs: &inputs,
                    scope: &scope,
                    resource: None,
                },
                timing(6).now,
            )
            .unwrap()
    };
    let (first_job, second_job) = thread::scope(|scope| {
        let first = scope.spawn(submit);
        let second = scope.spawn(submit);
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_eq!(first_job.id, second_job.id);
    let lease = lease(&fixture, &request);
    assert_eq!(lease.job, first_job.id);
    let database = &fixture.database;
    let reserve = || {
        database.write::<_, Error>(|tx| {
            authority::begin(
                tx,
                &ScopeSet::default_workspace(),
                &request,
                &lease,
                timing(7).now,
            )
        })
    };
    let (first, second) = thread::scope(|scope| {
        let first = scope.spawn(reserve);
        let second = scope.spawn(reserve);
        (
            first.join().unwrap().unwrap(),
            second.join().unwrap().unwrap(),
        )
    });
    assert_eq!(first, second);
    assert_eq!(fixture.count("graph_projection_builds"), 1);
}

#[test]
fn projection_reservation_lookup_excludes_migrated_null_job_builds() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    fixture
        .database
        .write::<_, Error>(|tx| {
            assert_eq!(
                authority::by_build(tx, &ScopeSet::default_workspace(), fixture.generation(),)?,
                None
            );
            Ok(())
        })
        .unwrap();
}
