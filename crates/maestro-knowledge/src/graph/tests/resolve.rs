//! Scoped identities and explicit, half-open validity ordering.

use crate::graph::{
    resolve::{
        EXACT_RESOLVER_VERSION, EqualityOnly, ValidityOrder, contains, resolve, resolve_snapshot,
    },
    rules::{Extractor as _, TableRule},
    tests::support::{markdown, rule_for, rule_text, source_at},
};
use maestro_kernel::{
    artifact::Digest,
    facts::{
        ClaimRecord, Decision, DecisionKind, Endpoint, EntityKind, EntityName, Mention,
        ResolutionSnapshot, ReviewRecord, ReviewState, Validity,
    },
};
use std::cmp::Ordering;

/// Synthetic numeric bounds; production makes no such ordering assumption.
struct Numeric;

impl ValidityOrder for Numeric {
    fn compare(&self, left: &str, right: &str) -> Option<Ordering> {
        Some(left.parse::<u64>().ok()?.cmp(&right.parse::<u64>().ok()?))
    }
}

#[test]
fn known_bounds_are_half_open_and_unknown_order_stays_unknown() {
    let bounds = Validity::Bounded {
        start: Some("2".to_owned()),
        end: Some("10".to_owned()),
    };
    for (point, expected) in [
        ("1", Some(false)),
        ("2", Some(true)),
        ("9", Some(true)),
        ("10", Some(false)),
        ("11", Some(false)),
        ("unorderable", None),
    ] {
        assert_eq!(contains(&bounds, point, &Numeric), expected);
    }
    assert_eq!(contains(&bounds, "2", &EqualityOnly), None);
    assert_eq!(contains(&bounds, "10", &EqualityOnly), None);
    assert_eq!(contains(&Validity::Unknown, "2026-01-01", &Numeric), None);
    assert_eq!(
        contains(
            &Validity::Bounded {
                start: None,
                end: None
            },
            "anything",
            &EqualityOnly
        ),
        Some(true)
    );
}

#[test]
fn moved_resolver_preserves_exact_spelling_and_kind_collisions() {
    let names = vec![
        EntityName {
            kind: EntityKind::Parameter,
            name: "Résumé".to_owned(),
        },
        EntityName {
            kind: EntityKind::Parameter,
            name: "Résumé".to_owned(),
        },
        EntityName {
            kind: EntityKind::Parameter,
            name: "resume".to_owned(),
        },
        EntityName {
            kind: EntityKind::Concept,
            name: "Résumé".to_owned(),
        },
    ];
    let resolved = resolve(&names);
    assert_eq!(resolved.len(), 3);
    assert!(resolved.iter().all(|entry| entry.colliding.len() == 2));
}

/// Two supporting copies plus a separately scoped occurrence, all literal claims.
fn snapshot() -> ResolutionSnapshot {
    let rule = TableRule::parse(&rule_text()).unwrap();
    let mut claims = Vec::new();
    for (collection, document) in [("one", "first"), ("one", "copy"), ("two", "other")] {
        let extraction = rule.extract(&source_at(&markdown(), document));
        let claim = extraction.claims.into_iter().next().unwrap();
        claims.push(ClaimRecord {
            id: Digest::of(document.as_bytes()),
            collection_id: collection.to_owned(),
            claim,
            review: ReviewState::Unreviewed,
            recorded_at: "2026-01-01".to_owned(),
        });
    }
    ResolutionSnapshot {
        resolver_version: EXACT_RESOLVER_VERSION.to_owned(),
        id: Digest::of(b"snapshot"),
        previous: None,
        sets: vec![],
        claims,
        history: vec![],
    }
}

/// An explicit action between two sourced subjects.
fn review(
    snapshot: &ResolutionSnapshot,
    left: usize,
    right: usize,
    kind: DecisionKind,
) -> ReviewRecord {
    ReviewRecord {
        reviewer: "person".to_owned(),
        decision: Decision {
            left: Mention {
                claim: snapshot.claims[left].id.clone(),
                endpoint: Endpoint::Subject,
            },
            right: Mention {
                claim: snapshot.claims[right].id.clone(),
                endpoint: Endpoint::Subject,
            },
            kind,
            reason: "reviewed source evidence".to_owned(),
        },
    }
}

#[test]
fn collections_stay_separate_and_duplicate_support_copies_are_one_group() {
    let snapshot = snapshot();
    let entities = resolve_snapshot(&snapshot).unwrap();
    assert_eq!(entities.len(), 2);
    let first = entities
        .iter()
        .find(|entity| entity.collection_id == "one")
        .unwrap();
    assert_eq!(first.mentions.len(), 2);
    assert_eq!(first.support_groups.len(), 1);
    assert!(
        entities
            .iter()
            .all(|entity| entity.subject.kind == EntityKind::Parameter)
    );
}

#[test]
fn reviewed_aliases_and_separation_are_reversible_across_collections() {
    let mut snapshot = snapshot();
    let original = resolve_snapshot(&snapshot).unwrap();
    snapshot
        .history
        .push(review(&snapshot, 0, 2, DecisionKind::Alias));
    assert_eq!(resolve_snapshot(&snapshot).unwrap().len(), 1);
    snapshot
        .history
        .push(review(&snapshot, 0, 2, DecisionKind::Separate));
    let restored = resolve_snapshot(&snapshot).unwrap();
    assert_eq!(restored, original);
}

#[test]
fn contradictions_and_supersession_never_remove_claims_or_supply_world_time() {
    let mut snapshot = snapshot();
    snapshot.claims[1]
        .claim
        .conditions
        .insert("platform".to_owned(), "synthetic".to_owned());
    snapshot.claims[1].claim.version = Validity::Bounded {
        start: Some("2".to_owned()),
        end: Some("10".to_owned()),
    };
    snapshot
        .history
        .push(review(&snapshot, 0, 1, DecisionKind::Supersedes));
    let before = snapshot.claims.clone();
    let entities = resolve_snapshot(&snapshot).unwrap();
    assert_eq!(snapshot.claims, before);
    assert_eq!(
        entities
            .iter()
            .map(|entity| entity.mentions.len())
            .sum::<usize>(),
        3
    );
    assert!(
        snapshot.claims.iter().all(|claim| contains(
            &claim.claim.world,
            &claim.recorded_at,
            &EqualityOnly
        )
        .is_none())
    );
}

#[test]
fn cyclic_aliases_are_ambiguous_not_arbitrarily_chosen() {
    let mut snapshot = snapshot();
    snapshot
        .history
        .push(review(&snapshot, 0, 2, DecisionKind::Alias));
    snapshot
        .history
        .push(review(&snapshot, 2, 0, DecisionKind::Alias));
    assert!(resolve_snapshot(&snapshot).is_err());
}

#[test]
fn exact_resolution_is_available_through_the_replaceable_port() {
    use crate::graph::resolve::{ExactResolver, IdentityResolver};
    let resolver: &dyn IdentityResolver = &ExactResolver;
    assert_eq!(resolver.version(), EXACT_RESOLVER_VERSION);
    let snapshot = snapshot();
    assert_eq!(
        resolver.resolve(&snapshot).unwrap(),
        resolve_snapshot(&snapshot).unwrap()
    );
}

#[test]
fn a_resolution_pin_names_the_algorithm_and_locks_entity_ids_and_memberships() {
    use serde_json::json;
    let mut snapshot = snapshot();
    for record in &mut snapshot.claims {
        record.claim.subject.name = "Résumé  COUNT".to_owned();
    }
    assert_eq!(snapshot.resolver_version, EXACT_RESOLVER_VERSION);
    let entities = resolve_snapshot(&snapshot).unwrap();
    let fixture = entities
        .iter()
        .map(|entity| {
            json!([
                entity.id.as_str(),
                entity.collection_id,
                entity
                    .mentions
                    .iter()
                    .map(|mention| mention.claim.as_str())
                    .collect::<Vec<_>>()
            ])
        })
        .collect::<Vec<_>>();
    assert_eq!(
        Digest::of(json!(fixture).to_string().as_bytes()).as_str(),
        "15e0b501d86a05f0d91002cd8501545d9d12c1343463b0621c5846fc48fcd9f2"
    );
    snapshot.resolver_version = "different-normalizer/2".to_owned();
    assert!(resolve_snapshot(&snapshot).is_err());
}

#[test]
fn separation_refuses_unmatched_pairs_including_after_retargeting() {
    let mut snapshot = snapshot();
    snapshot.claims[1].claim.subject.name = "third".to_owned();
    snapshot
        .history
        .push(review(&snapshot, 0, 1, DecisionKind::Alias));
    snapshot
        .history
        .push(review(&snapshot, 0, 2, DecisionKind::Separate));
    assert!(resolve_snapshot(&snapshot).is_err());
    snapshot.history.pop();
    snapshot
        .history
        .push(review(&snapshot, 0, 2, DecisionKind::Alias));
    snapshot
        .history
        .push(review(&snapshot, 0, 1, DecisionKind::Separate));
    assert!(resolve_snapshot(&snapshot).is_err());
    snapshot.history.pop();
    snapshot
        .history
        .push(review(&snapshot, 0, 2, DecisionKind::Separate));
    assert_eq!(resolve_snapshot(&snapshot).unwrap().len(), 3);
}

#[test]
fn reviewed_spelling_collisions_are_no_longer_unreviewed() {
    let mut snapshot = snapshot();
    snapshot.claims.truncate(2);
    snapshot.claims[0].claim.subject.name = "Café".to_owned();
    snapshot.claims[1].claim.subject.name = "cafe".to_owned();
    assert!(
        resolve_snapshot(&snapshot)
            .unwrap()
            .iter()
            .all(|entity| entity.colliding.len() == 1)
    );
    snapshot
        .history
        .push(review(&snapshot, 0, 1, DecisionKind::Alias));
    let entities = resolve_snapshot(&snapshot).unwrap();
    assert_eq!(entities.len(), 1);
    assert!(entities[0].colliding.is_empty());
}

#[test]
fn equality_only_compares_single_bound_endpoints() {
    for (start, end, expected) in [(Some("v2"), None, true), (None, Some("v2"), false)] {
        assert_eq!(
            contains(
                &Validity::Bounded {
                    start: start.map(str::to_owned),
                    end: end.map(str::to_owned),
                },
                "v2",
                &EqualityOnly
            ),
            Some(expected)
        );
    }
}

#[test]
fn entity_objects_supply_sourced_object_mentions() {
    use maestro_kernel::facts::Object;
    let mut snapshot = snapshot();
    let object = EntityName {
        kind: EntityKind::Concept,
        name: "object".to_owned(),
    };
    snapshot.claims[0].claim.object = Object::Entity(object.clone());
    let entities = resolve_snapshot(&snapshot).unwrap();
    let entity = entities
        .iter()
        .find(|entity| entity.subject == object)
        .unwrap();
    assert_eq!(
        entity.mentions,
        vec![Mention {
            claim: snapshot.claims[0].id.clone(),
            endpoint: Endpoint::Object,
        }]
    );
    assert_eq!(
        entity.support_groups,
        vec![snapshot.claims[0].claim.supports[0].quote_digest.clone()]
    );
}

/// Same-name concepts supported in three unrelated documentary namespaces.
fn namespace_snapshot() -> ResolutionSnapshot {
    let mut snapshot = snapshot();
    for (record, namespace) in snapshot.claims.iter_mut().zip(["north", "south", "west"]) {
        let text = format!("# Documentary namespace {namespace}\n\n{}", markdown());
        let rule = TableRule::parse(&rule_for(&text, &serde_json::json!({}))).unwrap();
        record.claim = rule.extract(&source_at(&text, namespace)).claims.remove(0);
        record.collection_id = "one".into();
    }
    snapshot
}

#[test]
fn same_name_namespace_separation_holds_the_unchanged_identity_for_review() {
    let mut snapshot = namespace_snapshot();
    snapshot.claims.truncate(2);
    let original = resolve_snapshot(&snapshot).unwrap();
    snapshot
        .history
        .push(review(&snapshot, 0, 1, DecisionKind::Separate));
    let result = resolve_snapshot(&snapshot);
    assert!(
        result.is_ok(),
        "a sourced collision must stay inspectable: {result:?}"
    );
    let held = result.unwrap();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].id, original[0].id);
    assert_eq!(held[0].mentions, original[0].mentions);
    assert_eq!(held[0].colliding, vec![held[0].subject.clone()]);
    snapshot
        .history
        .push(review(&snapshot, 1, 0, DecisionKind::Alias));
    assert_eq!(resolve_snapshot(&snapshot).unwrap(), original);
    snapshot
        .history
        .push(review(&snapshot, 1, 0, DecisionKind::Separate));
    assert_eq!(resolve_snapshot(&snapshot).unwrap(), held);
}

#[test]
fn same_name_review_clears_only_its_pair_not_other_namespace_collisions() {
    let mut snapshot = namespace_snapshot();
    snapshot.claims[2].collection_id = "one".into();
    let original = resolve_snapshot(&snapshot).unwrap();
    snapshot
        .history
        .push(review(&snapshot, 0, 1, DecisionKind::Separate));
    snapshot
        .history
        .push(review(&snapshot, 1, 2, DecisionKind::Separate));
    snapshot
        .history
        .push(review(&snapshot, 1, 0, DecisionKind::Alias));
    snapshot
        .history
        .push(review(&snapshot, 1, 2, DecisionKind::Supersedes));
    let result = resolve_snapshot(&snapshot);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(result.unwrap()[0].colliding.len(), 1);
    snapshot
        .history
        .push(review(&snapshot, 2, 1, DecisionKind::Alias));
    assert_eq!(resolve_snapshot(&snapshot).unwrap(), original);
}

#[test]
fn identity_reviews_require_two_distinct_mentions() {
    for kind in [DecisionKind::Alias, DecisionKind::Separate] {
        let mut snapshot = namespace_snapshot();
        snapshot.history.push(review(&snapshot, 0, 0, kind));
        assert!(
            resolve_snapshot(&snapshot).is_err(),
            "accepted self review: {kind:?}"
        );
    }
}

#[test]
fn cross_key_alias_does_not_mask_a_same_name_namespace_hold() {
    let mut snapshot = namespace_snapshot();
    snapshot.claims[2].collection_id = "two".into();
    snapshot
        .history
        .push(review(&snapshot, 0, 1, DecisionKind::Separate));
    snapshot
        .history
        .push(review(&snapshot, 0, 2, DecisionKind::Alias));
    let result = resolve_snapshot(&snapshot);
    assert!(result.is_ok(), "{result:?}");
    let entities = result.unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].collection_id, "two");
    assert_eq!(entities[0].colliding, vec![entities[0].subject.clone()]);
}
