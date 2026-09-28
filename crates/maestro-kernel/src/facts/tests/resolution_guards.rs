//! Frozen reviews, request coverage and rowid replacement regressions.

use super::support::{Scratch, granted, label, on, set_of};
use crate::{
    artifact::Digest,
    facts::{ClaimSet, ResolutionInput},
    scope::{WORKSPACE, collection_path},
};

#[test]
fn snapshot_review_states_are_frozen_while_live_claims_change() {
    use crate::facts::ReviewState;
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = granted(&database, "reviewer", WORKSPACE);
    let set = database
        .record_claim_set(&scopes, &set_of(vec![label()]))
        .unwrap();
    let input = ResolutionInput {
        resolver_version: "exact-test/1".to_owned(),
        sets: vec![set.id.clone()],
        previous: None,
        decisions: vec![],
    };
    let old = database
        .record_resolution(&scopes, "reviewer", &input)
        .unwrap();
    scratch
        .outside()
        .execute("UPDATE claims SET review_state = 'accepted'", [])
        .unwrap();
    assert_eq!(
        database.resolution(&scopes, "reviewer", &old.id).unwrap(),
        Some(old.clone())
    );
    assert_eq!(
        database
            .claim_set(&scopes, &set.id)
            .unwrap()
            .unwrap()
            .claims[0]
            .review,
        ReviewState::Accepted
    );
    let new = database
        .record_resolution(&scopes, "reviewer", &input)
        .unwrap();
    assert_ne!(old.id, new.id);
    assert_eq!(new.claims[0].review, ReviewState::Accepted);
}

#[test]
fn explicit_rowid_replacement_cannot_remove_a_snapshot() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = granted(&database, "reviewer", WORKSPACE);
    let set = database
        .record_claim_set(&scopes, &set_of(vec![label()]))
        .unwrap();
    let pin = database
        .record_resolution(
            &scopes,
            "reviewer",
            &ResolutionInput {
                resolver_version: "exact-test/1".to_owned(),
                sets: vec![set.id],
                previous: None,
                decisions: vec![],
            },
        )
        .unwrap();
    let outside = scratch.outside();
    outside
        .execute_batch("PRAGMA recursive_triggers = OFF")
        .unwrap();
    assert!(
        outside
            .execute(
                "INSERT OR REPLACE INTO graph_resolutions
        (rowid, id, reviewer, body) SELECT rowid, ?1, reviewer, body FROM graph_resolutions",
                [Digest::of(b"replacement").as_str()]
            )
            .is_err()
    );
    assert_eq!(
        database.resolution(&scopes, "reviewer", &pin.id).unwrap(),
        Some(pin)
    );
}

#[test]
fn record_resolution_requires_request_coverage_before_persisting() {
    use crate::facts::Error;
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = granted(&database, "writer", WORKSPACE);
    let left = database
        .record_claim_set(&scopes, &set_of(vec![label()]))
        .unwrap();
    let right = database
        .record_claim_set(
            &scopes,
            &ClaimSet {
                collection_id: "other".to_owned(),
                claims: vec![on("rev-o", label())],
            },
        )
        .unwrap();
    let request = granted(&database, "reviewer", &collection_path("graph"));
    granted(&database, "reviewer", &collection_path("other"));
    let input = ResolutionInput {
        resolver_version: "exact-test/1".to_owned(),
        sets: vec![left.id, right.id],
        previous: None,
        decisions: vec![],
    };
    assert!(matches!(
        database.record_resolution(&request, "reviewer", &input),
        Err(Error::Unauthorized)
    ));
    let count: i64 = scratch
        .outside()
        .query_row("SELECT count(*) FROM graph_resolutions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}
