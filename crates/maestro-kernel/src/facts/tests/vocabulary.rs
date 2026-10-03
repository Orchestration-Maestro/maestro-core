//! Closed vocabulary, entity endpoints and the existing scoped support authority.

use super::support::{Scratch, counts, execute, granted, label, relation_claim, set_of};
use crate::{
    artifact::Digest,
    facts::{EntityKind, Error, Predicate},
    scope::ScopeSet,
};

#[test]
fn every_kind_and_entity_predicate_round_trips_after_reopening() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = ScopeSet::default_workspace();
    let mut records = Vec::new();
    for kind in EntityKind::ALL {
        assert_eq!(EntityKind::parse(kind.as_str()), Some(kind));
        for predicate in Predicate::ALL {
            assert_eq!(Predicate::parse(predicate.as_str()), Some(predicate));
            if !predicate.is_claimable() || predicate.takes_literal() {
                continue;
            }
            let claim = relation_claim((kind, "p"), predicate, (kind, "c"));
            let record = database
                .record_claim_set(&scopes, &set_of(vec![claim.clone()]))
                .unwrap();
            assert_eq!(record.claims[0].claim, claim);
            records.push(record);
        }
    }
    drop(database);
    let reopened = scratch.open();
    for record in records {
        assert_eq!(
            reopened.claim_set(&scopes, &record.id).unwrap(),
            Some(record)
        );
    }
}

#[test]
fn entity_object_name_and_kind_each_distinguish_claim_identity() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = ScopeSet::default_workspace();
    let mut records = Vec::new();
    for object in [
        (EntityKind::Component, "c"),
        (EntityKind::Component, "different"),
        (EntityKind::Parameter, "c"),
    ] {
        let claim = relation_claim((EntityKind::Parameter, "p"), Predicate::Requires, object);
        let record = database
            .record_claim_set(&scopes, &set_of(vec![claim.clone()]))
            .unwrap();
        assert_eq!(record.claims[0].claim, claim);
        records.push(record);
    }
    assert_ne!(records[0].claims[0].id, records[1].claims[0].id);
    assert_ne!(records[0].claims[0].id, records[2].claims[0].id);
    assert_ne!(records[1].claims[0].id, records[2].claims[0].id);
    drop(database);
    let reopened = scratch.open();
    for record in records {
        assert_eq!(
            reopened.claim_set(&scopes, &record.id).unwrap(),
            Some(record)
        );
    }
}

#[test]
fn unlisted_spellings_aliases_and_wrong_endpoint_forms_are_refused_atomically() {
    for name in ["", "Setting", "parameter", "Api", "PRIVATE_KIND"] {
        assert_eq!(EntityKind::parse(name), None);
    }
    for name in ["", "USES", "requires", "PRIVATE_PREDICATE"] {
        assert_eq!(Predicate::parse(name), None);
    }
    let scratch = Scratch::new();
    let database = scratch.open();
    let entity = relation_claim(
        (EntityKind::Parameter, "p"),
        Predicate::Requires,
        (EntityKind::Component, "c"),
    );
    let mut cases = Vec::new();
    for predicate in [Predicate::AliasOf, Predicate::DefaultsTo] {
        let mut invalid = entity.clone();
        invalid.predicate = predicate;
        cases.push(invalid);
    }
    for predicate in Predicate::ALL
        .into_iter()
        .filter(|predicate| !predicate.takes_literal())
    {
        let mut invalid = label();
        invalid.predicate = predicate;
        cases.push(invalid);
    }
    cases.push(relation_claim(
        (EntityKind::Parameter, "p"),
        Predicate::Requires,
        (EntityKind::Component, ""),
    ));
    for invalid in cases {
        let error = database
            .record_claim_set(
                &ScopeSet::default_workspace(),
                &set_of(vec![entity.clone(), invalid]),
            )
            .unwrap_err();
        assert!(matches!(error, Error::Invalid(_)), "{error}");
        assert_eq!(counts(&scratch), [0; 4]);
    }
}

#[test]
fn entity_endpoints_are_collection_local_and_never_bypass_scope_or_support_checks() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let graph = granted(
        &database,
        "graph-reader",
        "workspace/default/collection/graph",
    );
    let other = granted(
        &database,
        "other-reader",
        "workspace/default/collection/other",
    );
    let claim = relation_claim(
        (EntityKind::Command, "start"),
        Predicate::Requires,
        (EntityKind::Component, "private-name"),
    );
    let set = set_of(vec![claim.clone()]);
    assert!(matches!(
        database.record_claim_set(&other, &set),
        Err(Error::Unauthorized)
    ));
    assert_eq!(counts(&scratch), [0; 4]);
    let record = database.record_claim_set(&graph, &set).unwrap();
    assert_eq!(database.claim_set(&other, &record.id).unwrap(), None);
    let mut foreign = claim.clone();
    foreign.supports[0].revision_id = "rev-o".to_owned();
    assert!(matches!(
        database.record_claim_set(&graph, &set_of(vec![foreign])),
        Err(Error::UnknownRevision { .. })
    ));
    let mut forged = claim;
    forged.supports[0].quote_digest = Digest::of(b"wrong quote");
    assert!(matches!(
        database.record_claim_set(&graph, &set_of(vec![forged])),
        Err(Error::QuoteMismatch { .. })
    ));
    assert_eq!(counts(&scratch), [1; 4]);
}

#[test]
fn sqlite_rejects_unlisted_kinds_predicates_and_mixed_object_columns() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for (subject, predicate, literal_type, lexeme, kind, name) in [
        (
            "'Setting'",
            "'REQUIRES'",
            "NULL",
            "NULL",
            "'Component'",
            "'c'",
        ),
        (
            "'Parameter'",
            "'USES'",
            "NULL",
            "NULL",
            "'Component'",
            "'c'",
        ),
        (
            "'Parameter'",
            "'ALIAS_OF'",
            "NULL",
            "NULL",
            "'Component'",
            "'c'",
        ),
        (
            "'Parameter'",
            "'REQUIRES'",
            "NULL",
            "NULL",
            "'Setting'",
            "'c'",
        ),
        ("'Parameter'", "'REQUIRES'", "'text'", "'x'", "NULL", "NULL"),
        (
            "'Parameter'",
            "'DEFAULTS_TO'",
            "NULL",
            "NULL",
            "'Component'",
            "'c'",
        ),
        (
            "'Parameter'",
            "'DEFAULTS_TO'",
            "'text'",
            "'x'",
            "'Component'",
            "'c'",
        ),
        (
            "'Parameter'",
            "'REQUIRES'",
            "NULL",
            "NULL",
            "'Component'",
            "NULL",
        ),
        (
            "'Parameter'",
            "'REQUIRES'",
            "NULL",
            "NULL",
            "'Component'",
            "''",
        ),
    ] {
        let sql = format!(
            "INSERT INTO claims (id, collection_id, subject_kind, subject_name,
            predicate, object_type, object_lexeme, object_kind, object_name, conditions_json,
            version_known, world_known, extractor, profile_digest, support_count)
            VALUES ('raw', 'graph', {subject}, 'p', {predicate}, {literal_type}, {lexeme},
            {kind}, {name}, '{{}}', 0, 0, 'test', 'profile', 1)"
        );
        let error = execute(&database, &sql).unwrap_err();
        assert!(
            format!("{error:?}").contains("CHECK constraint failed"),
            "{error}"
        );
    }
    assert_eq!(counts(&scratch), [0; 4]);
}
