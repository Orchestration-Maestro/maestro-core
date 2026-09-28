//! Immutable sourced resolution snapshots and current-grant checks.

use super::support::{Scratch, granted, label, on, second_revision, set_of};
use crate::{
    artifact::Digest,
    facts::{ClaimSet, Decision, DecisionKind, Endpoint, Mention, ResolutionInput},
    scope::{Right, WORKSPACE, collection_path},
    store::Database,
};

#[test]
fn sourced_reviews_are_reversible_without_changing_old_snapshots() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = granted(&database, "reviewer", WORKSPACE);
    second_revision(&database);
    let set = database
        .record_claim_set(&scopes, &set_of(vec![label(), on("rev-b", label())]))
        .unwrap();
    let left = Mention {
        claim: set.claims[0].id.clone(),
        endpoint: Endpoint::Subject,
    };
    let right = Mention {
        claim: set.claims[1].id.clone(),
        endpoint: Endpoint::Subject,
    };
    let input = ResolutionInput {
        resolver_version: "exact-test/1".to_owned(),
        sets: vec![set.id],
        previous: None,
        decisions: vec![Decision {
            left: left.clone(),
            right: right.clone(),
            kind: DecisionKind::Alias,
            reason: "reviewed source occurrences".to_owned(),
        }],
    };
    let first = database
        .record_resolution(&scopes, "reviewer", &input, &|_| Ok(()))
        .unwrap();
    assert_eq!(
        database
            .record_resolution(&scopes, "reviewer", &input, &|_| Ok(()))
            .unwrap(),
        first
    );
    let reversed = ResolutionInput {
        resolver_version: "exact-test/1".to_owned(),
        previous: Some(first.id.clone()),
        decisions: vec![Decision {
            left,
            right,
            kind: DecisionKind::Separate,
            reason: "reverse the earlier decision".to_owned(),
        }],
        ..input
    };
    let second = database
        .record_resolution(&scopes, "reviewer", &reversed, &|_| Ok(()))
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(second.history.len(), 2);
    assert_eq!(
        database
            .resolution(&scopes, "reviewer", &first.id)
            .unwrap()
            .unwrap(),
        first
    );
    drop(database);
    let database = Database::open_in(&scratch.0).unwrap();
    assert_eq!(
        database
            .resolution(&scopes, "reviewer", &second.id)
            .unwrap()
            .unwrap(),
        second
    );
    for sql in [
        "UPDATE graph_resolutions SET reviewer = 'changed'",
        "DELETE FROM graph_resolutions",
        "INSERT OR REPLACE INTO graph_resolutions
         SELECT id, previous_id, 'changed', body, recorded_at FROM graph_resolutions",
    ] {
        assert!(scratch.outside().execute(sql, []).is_err());
    }
}

#[test]
fn cross_collection_alias_and_history_disappear_when_either_grant_is_revoked() {
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
    for collection in ["graph", "other"] {
        granted(&database, "reviewer", &collection_path(collection));
    }
    let input = ResolutionInput {
        resolver_version: "exact-test/1".to_owned(),
        sets: vec![left.id, right.id],
        previous: None,
        decisions: vec![Decision {
            left: Mention {
                claim: left.claims[0].id.clone(),
                endpoint: Endpoint::Subject,
            },
            right: Mention {
                claim: right.claims[0].id.clone(),
                endpoint: Endpoint::Subject,
            },
            kind: DecisionKind::Alias,
            reason: "explicit cross-collection review".to_owned(),
        }],
    };
    let snapshot = database
        .record_resolution(&scopes, "reviewer", &input, &|_| Ok(()))
        .unwrap();
    let denied_request = database.visible("ungranted").unwrap();
    assert_eq!(
        database
            .resolution(&denied_request, "reviewer", &snapshot.id)
            .unwrap(),
        None
    );
    for collection in ["graph", "other"] {
        let scope = collection_path(collection).parse().unwrap();
        database
            .revoke("reviewer", &scope, Right::Read, "test")
            .unwrap();
        assert_eq!(
            database
                .resolution(&scopes, "reviewer", &snapshot.id)
                .unwrap(),
            None
        );
        assert_eq!(
            database
                .resolution(&scopes, "reviewer", &Digest::of(b"unknown"))
                .unwrap(),
            None
        );
        assert!(
            database
                .record_resolution(&scopes, "reviewer", &input, &|_| Ok(()))
                .is_err()
        );
        database
            .grant("reviewer", &scope, Right::Read, "test")
            .unwrap();
    }
}

#[test]
fn review_refuses_unsourced_endpoints_literals_and_empty_reasons() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = granted(&database, "reviewer", WORKSPACE);
    let set = database
        .record_claim_set(&scopes, &set_of(vec![label()]))
        .unwrap();
    let mention = Mention {
        claim: set.claims[0].id.clone(),
        endpoint: Endpoint::Subject,
    };
    let decision = Decision {
        left: mention.clone(),
        right: mention,
        kind: DecisionKind::Alias,
        reason: "source reviewed".to_owned(),
    };
    for bad in [
        Decision {
            left: Mention {
                claim: Digest::of(b"unknown"),
                endpoint: Endpoint::Subject,
            },
            ..decision.clone()
        },
        Decision {
            right: Mention {
                endpoint: Endpoint::Object,
                ..decision.right.clone()
            },
            ..decision.clone()
        },
        Decision {
            reason: String::new(),
            ..decision
        },
    ] {
        assert!(
            database
                .record_resolution(
                    &scopes,
                    "reviewer",
                    &ResolutionInput {
                        resolver_version: "exact-test/1".to_owned(),
                        sets: vec![set.id.clone()],
                        previous: None,
                        decisions: vec![bad]
                    },
                    &|_| Ok(())
                )
                .is_err()
        );
    }
    let count: i64 = scratch
        .outside()
        .query_row("SELECT count(*) FROM graph_resolutions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn upgrade_preserves_populated_claims_and_unknown_validity() {
    let scratch = Scratch::new();
    let database = scratch.open_before("0015_graph_resolution");
    let scopes = granted(&database, "reviewer", WORKSPACE);
    let set = database
        .record_claim_set(&scopes, &set_of(vec![label()]))
        .unwrap();
    drop(database);
    let database = Database::open_in(&scratch.0).unwrap();
    assert_eq!(
        database.claim_set(&scopes, &set.id).unwrap(),
        Some(set.clone())
    );
    let snapshot = database
        .record_resolution(
            &scopes,
            "reviewer",
            &ResolutionInput {
                resolver_version: "exact-test/1".to_owned(),
                sets: vec![set.id],
                previous: None,
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap();
    assert_eq!(snapshot.claims, set.claims);
}

#[test]
fn superseded_conflicting_defaults_keep_qualifiers_and_old_pins() {
    use crate::facts::{Literal, LiteralKind, Object, Validity};

    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = granted(&database, "reviewer", WORKSPACE);
    let mut conflicting = label();
    conflicting.object = Object::Literal(Literal {
        kind: LiteralKind::Text,
        lexeme: "different".to_owned(),
    });
    conflicting.version = Validity::Bounded {
        start: Some("2".to_owned()),
        end: Some("10".to_owned()),
    };
    conflicting
        .conditions
        .insert("platform".to_owned(), "synthetic".to_owned());
    let set = database
        .record_claim_set(&scopes, &set_of(vec![label(), conflicting.clone()]))
        .unwrap();
    let input = ResolutionInput {
        resolver_version: "exact-test/1".to_owned(),
        sets: vec![set.id.clone()],
        previous: None,
        decisions: vec![],
    };
    let old = database
        .record_resolution(&scopes, "reviewer", &input, &|_| Ok(()))
        .unwrap();
    let decision = Decision {
        left: Mention {
            claim: set.claims[0].id.clone(),
            endpoint: Endpoint::Subject,
        },
        right: Mention {
            claim: set.claims[1].id.clone(),
            endpoint: Endpoint::Subject,
        },
        kind: DecisionKind::Supersedes,
        reason: "a reviewed replacement, not a deletion".to_owned(),
    };
    let new = database
        .record_resolution(
            &scopes,
            "reviewer",
            &ResolutionInput {
                resolver_version: "exact-test/1".to_owned(),
                previous: Some(old.id.clone()),
                decisions: vec![decision],
                ..input
            },
            &|_| Ok(()),
        )
        .unwrap();
    assert_eq!(new.claims, old.claims);
    assert_eq!(new.claims.len(), 2);
    assert!(new.claims.iter().any(|record| record.claim == conflicting));
    assert!(
        new.claims
            .iter()
            .all(|record| record.claim.world == Validity::Unknown)
    );
    assert_eq!(
        database.resolution(&scopes, "reviewer", &old.id).unwrap(),
        Some(old)
    );
    assert_eq!(database.claim_set(&scopes, &set.id).unwrap(), Some(set));
}

#[test]
fn history_cannot_discard_sources_or_reviewer_attribution() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = granted(&database, "first", WORKSPACE);
    granted(&database, "second", WORKSPACE);
    let source = database
        .record_claim_set(&scopes, &set_of(vec![label()]))
        .unwrap();
    let other = database
        .record_claim_set(
            &scopes,
            &ClaimSet {
                collection_id: "other".to_owned(),
                claims: vec![on("rev-o", label())],
            },
        )
        .unwrap();
    let decision = Decision {
        left: Mention {
            claim: source.claims[0].id.clone(),
            endpoint: Endpoint::Subject,
        },
        right: Mention {
            claim: other.claims[0].id.clone(),
            endpoint: Endpoint::Subject,
        },
        kind: DecisionKind::Alias,
        reason: "cross-collection evidence reviewed".to_owned(),
    };
    let input = ResolutionInput {
        resolver_version: "exact-test/1".to_owned(),
        sets: vec![source.id.clone(), other.id],
        previous: None,
        decisions: vec![decision],
    };
    let old = database
        .record_resolution(&scopes, "first", &input, &|_| Ok(()))
        .unwrap();
    let next = ResolutionInput {
        resolver_version: "exact-test/1".to_owned(),
        previous: Some(old.id),
        decisions: vec![],
        ..input
    };
    let new = database
        .record_resolution(&scopes, "second", &next, &|_| Ok(()))
        .unwrap();
    assert_eq!(new.history[0].reviewer, "first");
    assert!(
        database
            .record_resolution(
                &scopes,
                "second",
                &ResolutionInput {
                    resolver_version: "exact-test/1".to_owned(),
                    sets: vec![source.id],
                    ..next
                },
                &|_| Ok(())
            )
            .is_err()
    );
    assert!(
        database
            .record_resolution(
                &scopes,
                "second",
                &ResolutionInput {
                    resolver_version: "exact-test/1".to_owned(),
                    sets: vec![],
                    previous: None,
                    decisions: vec![]
                },
                &|_| Ok(())
            )
            .is_err()
    );
}

#[test]
fn resolver_identity_is_frozen_and_changes_the_snapshot_digest() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = granted(&database, "reviewer", WORKSPACE);
    let set = database
        .record_claim_set(&scopes, &set_of(vec![label()]))
        .unwrap();
    let mut input = ResolutionInput {
        resolver_version: "normalizer/1".to_owned(),
        sets: vec![set.id],
        previous: None,
        decisions: vec![],
    };
    let first = database
        .record_resolution(&scopes, "reviewer", &input, &|_| Ok(()))
        .unwrap();
    input.resolver_version = "normalizer/2".to_owned();
    let second = database
        .record_resolution(&scopes, "reviewer", &input, &|_| Ok(()))
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(
        database
            .resolution(&scopes, "reviewer", &first.id)
            .unwrap()
            .unwrap()
            .resolver_version,
        "normalizer/1"
    );
    input.resolver_version.clear();
    assert!(
        database
            .record_resolution(&scopes, "reviewer", &input, &|_| Ok(()))
            .is_err()
    );
}
