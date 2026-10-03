//! Frozen snapshot membership and endpoint derivation for the single loader.
use super::super::{ProjectionScope, ProjectionSnapshot, content::tests::fact};
use crate::graph::resolve::{EXACT_RESOLVER_VERSION, resolve_snapshot};
use maestro_kernel::{
    artifact::Digest,
    facts::{ClaimSetRecord, Endpoint, Object, Predicate, ResolutionSnapshot, ReviewState},
};

fn inputs() -> (ClaimSetRecord, ResolutionSnapshot, ProjectionScope) {
    let fact = fact();
    let mut edge = fact.claim.clone();
    edge.id = Digest::of(b"edge");
    edge.claim.predicate = Predicate::Requires;
    edge.claim.object = Object::Entity(edge.claim.subject.clone());
    edge.review = ReviewState::Rejected;
    let set = ClaimSetRecord {
        id: Digest::of(b"set"),
        collection_id: "c".into(),
        claims: vec![fact.claim, edge],
    };
    let snapshot = ResolutionSnapshot {
        resolver_version: EXACT_RESOLVER_VERSION.into(),
        id: Digest::of(b"resolution"),
        previous: None,
        sets: vec![set.id.clone()],
        claims: set.claims.clone(),
        history: vec![],
    };
    (set, snapshot, fact.scope)
}

#[test]
fn import_preserves_complete_membership_and_frozen_reviews() {
    let (mut set, snapshot, scope) = inputs();
    set.claims[0].review = ReviewState::Accepted;
    let rows = ProjectionSnapshot::derive(&set, &snapshot, &scope).unwrap();
    assert_eq!(rows.facts.len(), 1);
    assert_eq!(rows.edges.len(), 1);
    assert_eq!(rows.facts[0].claim.review, ReviewState::Unreviewed);
    let entities = resolve_snapshot(&snapshot).unwrap();
    let expected = entities
        .iter()
        .find(|entity| {
            entity.mentions.iter().any(|mention| {
                mention.claim == set.claims[0].id && mention.endpoint == Endpoint::Subject
            })
        })
        .unwrap();
    assert_eq!(rows.facts[0].subject, expected.id);
    assert_eq!(rows.edges[0].id, set.claims[1].id);
    assert_eq!(rows.claim_set.claims, snapshot.claims);
    assert_eq!(rows.resolution_id, snapshot.id);
    assert_eq!(rows.resolver_version, EXACT_RESOLVER_VERSION);
}

#[test]
fn import_refuses_wrong_pin_incomplete_snapshot_and_changed_claim() {
    let (set, snapshot, scope) = inputs();
    let mut changed = snapshot.clone();
    changed.sets.clear();
    assert!(ProjectionSnapshot::derive(&set, &changed, &scope).is_err());
    changed = snapshot.clone();
    changed.claims.pop();
    assert!(ProjectionSnapshot::derive(&set, &changed, &scope).is_err());
    changed = snapshot;
    changed.claims[0].claim.subject.name = "changed".into();
    assert!(ProjectionSnapshot::derive(&set, &changed, &scope).is_err());
    let (_, original, _) = inputs();
    let wrong = ProjectionScope {
        collection_id: "other".into(),
        ..scope.clone()
    };
    assert!(ProjectionSnapshot::derive(&set, &original, &wrong).is_err());
    for generation_id in [0, -1] {
        let wrong = ProjectionScope {
            generation_id,
            ..scope.clone()
        };
        assert!(ProjectionSnapshot::derive(&set, &original, &wrong).is_err());
    }
    let mut foreign = original;
    foreign.claims[0].collection_id = "other".into();
    assert!(ProjectionSnapshot::derive(&set, &foreign, &scope).is_err());
}

#[test]
fn import_keeps_unicode_quotes_newlines_and_bound_looking_literals() {
    let (mut set, mut snapshot, scope) = inputs();
    let Object::Literal(literal) = &mut set.claims[0].claim.object else {
        panic!()
    };
    literal.lexeme = "é雪 '\"\nMATCH (n) DELETE n; $value".into();
    snapshot.claims = set.claims.clone();
    let rows = ProjectionSnapshot::derive(&set, &snapshot, &scope).unwrap();
    assert_eq!(rows.facts[0].claim.claim, set.claims[0].claim);
    assert_eq!(rows.edges.len(), 1);
}

#[test]
fn import_retains_supersession_history_and_uses_reviewed_alias_endpoints() {
    use maestro_kernel::facts::{Decision, DecisionKind, Mention, ReviewRecord};
    let (mut set, mut snapshot, scope) = inputs();
    set.claims[1].claim.subject.name = "reviewed target".into();
    snapshot.claims = set.claims.clone();
    let left = Mention {
        claim: set.claims[0].id.clone(),
        endpoint: Endpoint::Subject,
    };
    let right = Mention {
        claim: set.claims[1].id.clone(),
        endpoint: Endpoint::Subject,
    };
    for kind in [DecisionKind::Alias, DecisionKind::Supersedes] {
        snapshot.history.push(ReviewRecord {
            reviewer: "synthetic reviewer".into(),
            decision: Decision {
                left: left.clone(),
                right: right.clone(),
                kind,
                reason: "sourced review".into(),
            },
        });
    }
    let rows = ProjectionSnapshot::derive(&set, &snapshot, &scope).unwrap();
    assert_eq!(rows.history, snapshot.history);
    assert_eq!(rows.claim_set.claims, snapshot.claims);
    assert_eq!(rows.facts[0].subject, rows.edges[0].source);
    let original = ProjectionSnapshot::derive(&inputs().0, &inputs().1, &scope).unwrap();
    assert_ne!(rows.facts[0].subject, original.facts[0].subject);
}

#[test]
fn import_reads_only_explicit_visible_kernel_pins() {
    use crate::graph::{descriptors::tests_source::Authority, resolve::validate_snapshot};
    use maestro_kernel::{
        facts::ResolutionInput,
        scope::{Right, ScopeSet, WORKSPACE},
    };
    let authority = Authority::new();
    let kernel = &authority.database;
    let scope = ProjectionScope {
        collection_id: authority.pin.collection_id.clone(),
        generation_id: authority.pin.generation_id,
    };
    let set = kernel
        .graph_attachment(&authority.scopes, scope.generation_id)
        .unwrap()
        .unwrap()
        .claim_set_id;
    let snapshot = kernel
        .record_resolution(
            &authority.scopes,
            "builder",
            &ResolutionInput {
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                sets: vec![set.clone()],
                previous: None,
                decisions: vec![],
            },
            &validate_snapshot,
        )
        .unwrap();
    let read = |request: &ScopeSet, principal: &str, pins: (&Digest, &Digest)| {
        ProjectionSnapshot::read(kernel, request, principal, pins, &scope)
    };
    let found = read(&authority.scopes, "builder", (&set, &snapshot.id)).unwrap();
    assert_eq!(found.claim_set.claims, snapshot.claims);
    assert_eq!(found.resolution_id, snapshot.id);
    assert_eq!(found.scope, scope);
    kernel.verify_generation(scope.generation_id, 1).unwrap();
    kernel.publish_generation(scope.generation_id).unwrap();
    kernel.retire_generation(scope.generation_id).unwrap();
    let retired = read(&authority.scopes, "builder", (&set, &snapshot.id)).unwrap();
    assert_eq!(retired.claim_set, found.claim_set);
    assert_eq!(retired.edges, found.edges);
    assert_eq!(retired.facts, found.facts);
    assert!(
        read(
            &authority.scopes,
            "builder",
            (&set, &Digest::of(b"missing snapshot"))
        )
        .is_err()
    );
    assert!(
        read(
            &authority.scopes,
            "builder",
            (&Digest::of(b"missing set"), &snapshot.id)
        )
        .is_err()
    );
    assert!(
        read(
            &kernel.visible("ungranted").unwrap(),
            "builder",
            (&set, &snapshot.id)
        )
        .is_err()
    );
    assert!(read(&authority.scopes, "ungranted", (&set, &snapshot.id)).is_err());
    kernel
        .revoke("builder", &WORKSPACE.parse().unwrap(), Right::Read, "test")
        .unwrap();
    assert!(read(&authority.scopes, "builder", (&set, &snapshot.id)).is_err());
}

#[test]
fn import_refuses_unregistered_resolver_version() {
    let (set, mut snapshot, scope) = inputs();
    snapshot.resolver_version = "unknown-resolver/2".into();
    assert!(ProjectionSnapshot::derive(&set, &snapshot, &scope).is_err());
}

#[test]
fn import_refuses_foreign_entity_member_and_wrong_entity_only_scope() {
    let (mut set, mut snapshot, mut scope) = inputs();
    set.claims.remove(0);
    snapshot.claims = set.claims.clone();
    scope.collection_id = "other".into();
    assert!(ProjectionSnapshot::derive(&set, &snapshot, &scope).is_err());
    scope.collection_id = set.collection_id.clone();
    snapshot.claims[0].collection_id = "other".into();
    assert!(ProjectionSnapshot::derive(&set, &snapshot, &scope).is_err());
}

#[test]
fn import_empty_membership_still_requires_valid_generation_and_keeps_pins() {
    let (mut set, mut snapshot, mut scope) = inputs();
    set.claims.clear();
    snapshot.claims.clear();
    let rows = ProjectionSnapshot::derive(&set, &snapshot, &scope).unwrap();
    assert_eq!(rows.scope, scope);
    assert_eq!(rows.claim_set, set);
    assert!(rows.edges.is_empty());
    assert!(rows.facts.is_empty());
    scope.generation_id = 0;
    assert!(ProjectionSnapshot::derive(&set, &snapshot, &scope).is_err());
}

#[test]
fn import_loads_only_selected_set_members_even_when_resolution_has_history_sets() {
    let (set, mut snapshot, scope) = inputs();
    let mut historical = snapshot.claims[0].clone();
    historical.id = Digest::of(b"historical claim");
    historical.claim.subject.name = "historical subject".into();
    snapshot.claims.push(historical);
    snapshot.sets.push(Digest::of(b"historical set"));
    let rows = ProjectionSnapshot::derive(&set, &snapshot, &scope).unwrap();
    assert_eq!(rows.claim_set, set);
    assert_eq!(rows.facts.len(), 1);
    assert_eq!(rows.edges.len(), 1);
}
