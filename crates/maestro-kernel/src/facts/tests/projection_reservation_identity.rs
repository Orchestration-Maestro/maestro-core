//! Migrated identities and concurrent submissions keep one build/job authority.
use super::{
    projection_reservation::{lease, request},
    projection_schema_support::Fixture,
    support::timing,
};
use crate::{
    facts::{
        Error, projection_publication, projection_records, projection_records::decode_identity,
        projection_reservation as authority,
    },
    job::NewJob,
    scope::{Scope, ScopeSet},
};
use std::{collections::BTreeSet, thread};

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

#[test]
fn projection_reservation_public_admission_and_lookup_share_authority() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let request = request(&fixture, None);
    let lease = lease(&fixture, &request);
    let scopes = ScopeSet::default_workspace();
    let reserved = fixture
        .database
        .begin_projection_build(&scopes, &request, &lease, timing(7).now)
        .unwrap();
    assert_eq!(
        fixture
            .database
            .projection_build(&scopes, reserved.build_id)
            .unwrap(),
        Some(reserved.clone())
    );
    assert_eq!(
        fixture
            .database
            .projection_build_for_job(&scopes, lease.job)
            .unwrap(),
        Some(reserved.clone())
    );
    assert_eq!(
        fixture
            .database
            .begin_projection_build(&scopes, &request, &lease, timing(7).now)
            .unwrap(),
        reserved
    );
}

#[test]
fn projection_reservation_receipt_decoder_keeps_exact_build_identity() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    let scopes = ScopeSet::default_workspace();
    let previous = fixture.head();
    let request = request(&fixture, Some(previous));
    let lease = lease(&fixture, &request);
    let reserved = fixture
        .database
        .begin_projection_build(&scopes, &request, &lease, timing(7).now)
        .unwrap();
    assert_ne!(reserved.build_id, fixture.generation());
    let mut receipt = fixture.receipt.clone();
    receipt.identity.schema_version = "maestro-typed-edges/3".into();
    receipt.identity.file_name = "new.lbdb".into();
    receipt.identity.build_id = reserved.build_id;
    fixture
        .database
        .write::<_, Error>(|tx| {
            projection_publication::publish(tx, &scopes, &receipt, &lease, timing(7).now)?;
            let record =
                projection_records::by_build(tx, &scopes, fixture.generation(), reserved.build_id)?
                    .unwrap();
            assert_eq!(record.identity.build_id, reserved.build_id);
            Ok(())
        })
        .unwrap();
    receipt.identity.build_id = reserved.build_id;
    assert_eq!(
        fixture
            .database
            .projection_build_receipt(&scopes, fixture.generation(), reserved.build_id)
            .unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(
        fixture
            .database
            .projection_build_receipt_identity(&scopes, fixture.generation(), reserved.build_id)
            .unwrap(),
        Some(receipt.identity)
    );
    let denied = ScopeSet::new(BTreeSet::new());
    for (visible, generation) in [
        (&denied, fixture.generation()),
        (&scopes, fixture.generation() + 1),
    ] {
        assert_eq!(
            fixture
                .database
                .projection_build_receipt(visible, generation, reserved.build_id)
                .unwrap(),
            None
        );
        assert_eq!(
            fixture
                .database
                .projection_build_receipt_identity(visible, generation, reserved.build_id)
                .unwrap(),
            None
        );
    }
    assert_eq!(
        fixture
            .database
            .projection_build(&denied, reserved.build_id)
            .unwrap(),
        None
    );
    assert_eq!(
        fixture
            .database
            .projection_build_for_job(&denied, lease.job)
            .unwrap(),
        None
    );
}

#[test]
fn projection_reservation_decoder_refuses_nonpositive_build_identity() {
    let fixture = Fixture::new(false);
    let receipt = &fixture.receipt;
    let row = || {
        (
            receipt.identity.collection_id.clone(),
            receipt.identity.claim_set_id.as_str().into(),
            receipt.identity.file_name.clone(),
            receipt.identity.schema_version.clone(),
            0,
            0,
            1,
            receipt.identity.content_digest.as_str().into(),
            Some(receipt.resolution_id.as_str().into()),
            Some(receipt.resolver_version.clone()),
            Some(receipt.settings_identity.as_str().into()),
            Some(receipt.frozen_lock.as_str().into()),
        )
    };
    for build in [0, -1] {
        assert!(matches!(
            decode_identity(fixture.generation(), build, row()),
            Err(Error::Conflict(_))
        ));
    }
    assert_eq!(
        decode_identity(fixture.generation(), 9, row())
            .unwrap()
            .0
            .build_id,
        9
    );
}
