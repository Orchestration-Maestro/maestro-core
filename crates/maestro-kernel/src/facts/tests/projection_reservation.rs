//! Standalone 0031 typed reservation proofs; runtime registration stays off.
use super::{projection_schema_support::Fixture, support::timing};
use crate::{
    artifact::Digest,
    facts::{
        Error,
        projection_reservation::{self as authority, Request, Reservation},
    },
    job::{self, JobState, Lease, NewJob},
    scope::{Scope, ScopeSet},
};
use serde_json::json;
use std::{collections::BTreeSet, thread};

/// Frozen request from synthetic retained authority, without opening native files.
fn request(fixture: &Fixture, previous: Option<i64>) -> Request {
    let receipt = &fixture.receipt;
    Request {
        collection_id: receipt.identity.collection_id.clone(),
        generation_id: fixture.generation(),
        claim_set_id: receipt.identity.claim_set_id.clone(),
        resolution_id: receipt.resolution_id.clone(),
        resolver_version: receipt.resolver_version.clone(),
        settings_identity: receipt.settings_identity.clone(),
        frozen_lock: receipt.frozen_lock.clone(),
        expected_active_build_id: previous,
    }
}

/// Submit and lease the canonical attempt, using existing job timing.
fn lease(fixture: &Fixture, request: &Request) -> Lease {
    let scope: Scope = format!("workspace/default/collection/{}", request.collection_id)
        .parse()
        .unwrap();
    let inputs = request.inputs();
    let job = fixture
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
        .unwrap();
    job.lease.unwrap_or_else(|| {
        fixture
            .database
            .take_job(job.id, "projector", timing(6).now, timing(6).term)
            .unwrap()
    })
}

/// Reservation under a refreshed caller's scope ceiling.
fn begin(fixture: &Fixture, request: &Request, lease: &Lease) -> Result<Reservation, Error> {
    fixture.database.write(|tx| {
        authority::begin(
            tx,
            &ScopeSet::default_workspace(),
            request,
            lease,
            timing(7).now,
        )
    })
}

#[test]
fn projection_reservation_replays_exact_inputs_and_scopes_both_lookups() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let request = request(&fixture, None);
    let lease = lease(&fixture, &request);
    let unchanged = [
        "generations",
        "graph_attachments",
        "jobs",
        "events",
        "generation_search",
    ]
    .map(|table| {
        (
            table,
            super::projection_build_upgrade::rows(&fixture, table),
        )
    });
    assert_eq!(
        request.inputs(),
        json!({
            "schema": "graph-project/2", "generation": fixture.generation(),
            "claim_set": fixture.receipt.identity.claim_set_id.as_str(),
            "resolution": fixture.receipt.resolution_id.as_str(),
            "resolver": "maestro-exact-resolution/1",
            "settings": fixture.receipt.settings_identity.as_str(),
            "lock": fixture.receipt.frozen_lock.as_str(), "format": "maestro-typed-edges/3",
            "expected_active_build": null,
        })
    );
    let reserved = begin(&fixture, &request, &lease).unwrap();
    assert!(reserved.build_id > 0);
    assert_eq!(reserved.request, request);
    assert_eq!(reserved.job, lease.job);
    assert_eq!(begin(&fixture, &request, &lease).unwrap(), reserved);
    assert_eq!(fixture.count("graph_projection_builds"), 1);
    assert_eq!(fixture.count("graph_projection_receipts"), 0);
    assert_eq!(fixture.count("graph_projection_active"), 0);
    for (table, before) in unchanged {
        assert_eq!(
            super::projection_build_upgrade::rows(&fixture, table),
            before,
            "{table}"
        );
    }
    fixture
        .database
        .write::<_, Error>(|tx| {
            let all = ScopeSet::default_workspace();
            assert_eq!(
                authority::by_build(tx, &all, reserved.build_id)?,
                Some(reserved.clone())
            );
            assert_eq!(
                authority::by_job(tx, &all, lease.job)?,
                Some(reserved.clone())
            );
            let denied = ScopeSet::new(BTreeSet::default());
            assert_eq!(authority::by_build(tx, &denied, reserved.build_id)?, None);
            assert_eq!(authority::by_job(tx, &denied, lease.job)?, None);
            assert_eq!(authority::by_build(tx, &all, i64::MAX)?, None);
            assert!(matches!(
                authority::validate(tx, &denied, reserved.build_id, &lease, timing(7).now),
                Err(Error::Unauthorized)
            ));
            assert_eq!(
                authority::validate(tx, &all, reserved.build_id, &lease, timing(7).now)?,
                reserved
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn projection_reservation_rejects_each_changed_frozen_field_and_legacy_job() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let original = request(&fixture, None);
    let lease = lease(&fixture, &original);
    begin(&fixture, &original, &lease).unwrap();
    for field in 0..8 {
        let mut changed = original.clone();
        match field {
            0 => changed.collection_id = "other".into(),
            1 => changed.generation_id += 1,
            2 => changed.claim_set_id = Digest::of(b"other"),
            3 => changed.resolution_id = Digest::of(b"other"),
            4 => changed.resolver_version = "unsupported".into(),
            5 => changed.settings_identity = Digest::of(b"other"),
            6 => changed.frozen_lock = Digest::of(b"other"),
            _ => changed.expected_active_build_id = Some(1),
        }
        assert!(begin(&fixture, &changed, &lease).is_err(), "field {field}");
    }
    let scope: Scope = "workspace/default/collection/graph".parse().unwrap();
    let job = fixture
        .database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.project",
                inputs: &json!({"generation": original.generation_id}),
                scope: &scope,
                resource: None,
            },
            timing(6).now,
        )
        .unwrap();
    let legacy = fixture
        .database
        .take_job(job.id, "legacy", timing(6).now, timing(6).term)
        .unwrap();
    assert!(matches!(
        begin(&fixture, &original, &legacy),
        Err(Error::Unauthorized)
    ));
    assert_eq!(fixture.count("graph_projection_builds"), 1);
}

#[test]
fn projection_reservation_rechecks_expiry_takeover_and_ended_job() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let request = request(&fixture, None);
    let mut held = lease(&fixture, &request);
    let reserved = begin(&fixture, &request, &held).unwrap();
    // The persisted expiry, not a forged caller copy, is authoritative.
    held.expires = "9999-12-31T23:59:59.999Z".into();
    fixture
        .database
        .write::<_, Error>(|tx| {
            assert!(matches!(
                authority::validate(
                    tx,
                    &ScopeSet::default_workspace(),
                    reserved.build_id,
                    &held,
                    timing(66).now
                ),
                Err(Error::Job(job::Error::Lost { .. }))
            ));
            Ok(())
        })
        .unwrap();
    let replacement = fixture
        .database
        .take_job(held.job, "new", timing(100).now, timing(100).term)
        .unwrap();
    assert!(matches!(
        begin(&fixture, &request, &held),
        Err(Error::Job(job::Error::Lost { .. }))
    ));
    assert_eq!(
        fixture
            .database
            .write::<_, Error>(|tx| authority::validate(
                tx,
                &ScopeSet::default_workspace(),
                reserved.build_id,
                &replacement,
                timing(101).now
            ))
            .unwrap(),
        reserved
    );
    fixture
        .database
        .complete_job(&replacement, JobState::Failed, &json!({}))
        .unwrap();
    assert!(matches!(
        begin(&fixture, &request, &replacement),
        Err(Error::Job(job::Error::Lost { .. }))
    ));
    let fresh = lease(&fixture, &request);
    let next = begin(&fixture, &request, &fresh).unwrap();
    assert_ne!(next.build_id, reserved.build_id);
    assert_ne!(next.job, reserved.job);
    fixture
        .database
        .write::<_, Error>(|tx| {
            assert!(matches!(
                authority::validate(
                    tx,
                    &ScopeSet::default_workspace(),
                    reserved.build_id,
                    &fresh,
                    timing(7).now
                ),
                Err(Error::Unauthorized)
            ));
            Ok(())
        })
        .unwrap();
}

#[test]
fn projection_reservation_replay_refuses_changed_head_receipt_or_state() {
    for boundary in ["head", "receipt", "retired", "failed", "building"] {
        let fixture = Fixture::new(true);
        fixture.migrate();
        let predecessor = fixture.head();
        let request = request(&fixture, Some(predecessor));
        let lease = lease(&fixture, &request);
        let reserved = begin(&fixture, &request, &lease).unwrap();
        match boundary {
            "head" => {
                let next = fixture.reserve(Some(predecessor));
                fixture.insert_receipt(next, "next.db").unwrap();
                fixture.advance(next).unwrap();
            }
            "receipt" => {
                fixture
                    .insert_receipt(reserved.build_id, "published.db")
                    .unwrap();
            }
            state => fixture.set_state(state),
        }
        assert!(begin(&fixture, &request, &lease).is_err(), "{boundary}");
        fixture
            .database
            .write::<_, Error>(|tx| {
                assert!(
                    authority::validate(
                        tx,
                        &ScopeSet::default_workspace(),
                        reserved.build_id,
                        &lease,
                        timing(7).now
                    )
                    .is_err(),
                    "{boundary}"
                );
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn projection_reservation_identical_concurrent_requests_deduplicate() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let request = request(&fixture, None);
    let lease = lease(&fixture, &request);
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
fn projection_reservation_requires_resourceless_exact_project_inputs() {
    for field in ["resource", "extra", "format", "schema", "kind"] {
        let fixture = Fixture::new(false);
        fixture.migrate();
        let request = request(&fixture, None);
        let mut inputs = request.inputs();
        match field {
            "extra" => inputs["unexpected"] = json!(true),
            "format" => inputs["format"] = json!("maestro-typed-edges/2"),
            "schema" => inputs["schema"] = json!("graph-project/1"),
            _ => {}
        }
        let scope: Scope = "workspace/default/collection/graph".parse().unwrap();
        let job = fixture
            .database
            .submit_job(
                &NewJob {
                    kind: if field == "kind" {
                        "other"
                    } else {
                        "knowledge.graph.project"
                    },
                    inputs: &inputs,
                    scope: &scope,
                    resource: (field == "resource").then_some("foreign-resource"),
                },
                timing(6).now,
            )
            .unwrap();
        let lease = fixture
            .database
            .take_job(job.id, "holder", timing(6).now, timing(6).term)
            .unwrap();
        assert!(
            matches!(begin(&fixture, &request, &lease), Err(Error::Unauthorized)),
            "{field}"
        );
        assert_eq!(fixture.count("graph_projection_builds"), 0);
    }
}

#[test]
fn projection_reservation_replacement_allows_new_settings_but_keeps_resolution() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    fixture.set_state("published");
    let predecessor = fixture.head();
    let mut replacement = request(&fixture, Some(predecessor));
    replacement.settings_identity = Digest::of(b"new settings");
    replacement.frozen_lock = Digest::of(b"new complete lock");
    let lease = lease(&fixture, &replacement);
    let reserved = begin(&fixture, &replacement, &lease).unwrap();
    assert!(reserved.build_id > predecessor);
    assert_eq!(fixture.head(), predecessor);
    assert_eq!(fixture.count("graph_projection_receipts"), 1);
    replacement.resolution_id = Digest::parse(&fixture.alternate_resolution()).unwrap();
    let other = super::projection_reservation::lease(&fixture, &replacement);
    assert!(matches!(
        begin(&fixture, &replacement, &other),
        Err(Error::Unauthorized)
    ));
    assert_eq!(fixture.count("graph_projection_builds"), 2);
}

#[test]
fn projection_reservation_rechecks_every_resolution_set_against_scope_ceiling() {
    use crate::facts::{ClaimSet, ResolutionInput};
    let fixture = Fixture::new(false);
    fixture.migrate();
    let all = ScopeSet::default_workspace();
    let hidden = fixture
        .database
        .record_claim_set(
            &all,
            &ClaimSet {
                collection_id: "other".into(),
                claims: vec![super::support::on("rev-o", super::support::label())],
            },
        )
        .unwrap();
    let mut request = request(&fixture, None);
    request.resolution_id = fixture
        .database
        .record_resolution(
            &all,
            "projection",
            &ResolutionInput {
                resolver_version: request.resolver_version.clone(),
                sets: vec![request.claim_set_id.clone(), hidden.id],
                previous: None,
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap()
        .id;
    let lease = lease(&fixture, &request);
    let limited = ScopeSet::new(BTreeSet::from(["workspace/default/collection/graph"
        .parse()
        .unwrap()]));
    fixture
        .database
        .write::<_, Error>(|tx| {
            assert!(authority::begin(tx, &limited, &request, &lease, timing(7).now).is_err());
            assert!(matches!(
                authority::begin(
                    tx,
                    &ScopeSet::new(BTreeSet::new()),
                    &request,
                    &lease,
                    timing(7).now
                ),
                Err(Error::Unauthorized)
            ));
            Ok(())
        })
        .unwrap();
    assert_eq!(fixture.count("graph_projection_builds"), 0);
    let reserved = begin(&fixture, &request, &lease).unwrap();
    fixture
        .database
        .write::<_, Error>(|tx| {
            assert!(
                authority::validate(tx, &limited, reserved.build_id, &lease, timing(7).now)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn projection_reservation_stored_pin_corruption_is_not_an_input_mismatch() {
    for column in [
        "claim_set_id",
        "resolution_id",
        "resolver_version",
        "settings_identity",
        "frozen_lock",
        "schema_version",
        "project_job_id",
    ] {
        let fixture = Fixture::new(false);
        fixture.migrate();
        let request = request(&fixture, None);
        let lease = lease(&fixture, &request);
        let reserved = begin(&fixture, &request, &lease).unwrap();
        fixture
            .connection
            .execute_batch(
                "DROP TRIGGER graph_projection_builds_never_changed;
            PRAGMA ignore_check_constraints=ON; PRAGMA foreign_keys=OFF;",
            )
            .unwrap();
        fixture
            .connection
            .execute(
                &format!(
                    "UPDATE graph_projection_builds SET {column} = 'malformed' WHERE build_id = ?1"
                ),
                [reserved.build_id],
            )
            .unwrap();
        fixture
            .database
            .write::<_, Error>(|tx| {
                assert!(
                    matches!(
                        authority::by_build(tx, &ScopeSet::default_workspace(), reserved.build_id),
                        Err(Error::Conflict(_))
                    ),
                    "{column}"
                );
                Ok(())
            })
            .unwrap();
    }
}
