//! Admitting claims: a whole set or nothing, unreviewed, recorded once by
//! their content, read back equal after a reopen, and refused when their
//! form or their scope is wrong.
//!
//! Architecture 02 §8.2 imposes no predicate domain or range limits. Refusing
//! event satisfaction disguised as `DEPENDS_ON` between listed kinds belongs
//! to G18's constrained gateway and review, not name matching here. A narrower
//! authority rule needs an ADR under FR-S2-024.

use super::support::{
    COLLECTION, LABEL_ROW, Scratch, counts, default_claim, execute, granted, label, quoting,
    retries, set_of, span_of,
};
use crate::{
    artifact::Digest,
    facts::{Claim, ClaimSet, EntityKind, Error, LiteralKind, ReviewState, Validity},
    scope::ScopeSet,
    store,
};
use rusqlite::{Error as SqliteError, ErrorCode, ffi::SQLITE_CONSTRAINT_CHECK};
use std::collections::BTreeMap;

#[test]
fn a_claim_set_is_admitted_unreviewed_in_order_and_reads_back_equal() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let set = set_of(vec![retries(), label()]);
    let record = database
        .record_claim_set(&ScopeSet::default_workspace(), &set)
        .unwrap();
    assert_eq!(record.collection_id, COLLECTION);
    let claims: Vec<_> = record
        .claims
        .iter()
        .map(|claim| claim.claim.clone())
        .collect();
    assert_eq!(claims, set.claims);
    for claim in &record.claims {
        assert_eq!(claim.collection_id, COLLECTION);
        assert_eq!(claim.review, ReviewState::Unreviewed);
        assert_eq!(claim.recorded_at.len(), "2026-09-28T00:00:00.000Z".len());
    }
    assert_eq!(counts(&scratch), [2, 2, 1, 2]);
    assert_eq!(
        database
            .claim_set(&ScopeSet::default_workspace(), &record.id)
            .unwrap(),
        Some(record)
    );
}

#[test]
fn claim_and_set_ids_are_the_digests_of_their_canonical_form() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut claim = label();
    claim.conditions = BTreeMap::from([("platform".to_owned(), "linux".to_owned())]);
    claim.version = Validity::Bounded {
        start: Some("9.0.22".to_owned()),
        end: None,
    };
    let record = database
        .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![claim]))
        .unwrap();
    let span = span_of(LABEL_ROW);
    let canonical = format!(
        concat!(
            r#"["maestro-claim/1","graph","Parameter","label","DEFAULTS_TO","text","café","#,
            r#"[["platform","linux"]],["9.0.22",null],null,"synthetic-defaults/1","{}","#,
            r#"[["rev-a","block-label",{},{},"{}"]]]"#
        ),
        Digest::of(b"synthetic-defaults/1").as_str(),
        span.start,
        span.end,
        Digest::of(LABEL_ROW.as_bytes()).as_str()
    );
    let claim_id = Digest::of(canonical.as_bytes());
    assert_eq!(record.claims[0].id, claim_id);
    let set = format!(
        r#"["maestro-claim-set/1","graph",["{}"]]"#,
        claim_id.as_str()
    );
    assert_eq!(record.id, Digest::of(set.as_bytes()));
}

#[test]
fn replaying_a_set_returns_its_record_and_records_nothing_more() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = ScopeSet::default_workspace();
    let set = set_of(vec![label(), retries()]);
    let first = database.record_claim_set(&scopes, &set).unwrap();
    let again = database.record_claim_set(&scopes, &set).unwrap();
    assert_eq!(again, first);
    assert_eq!(counts(&scratch), [2, 2, 1, 2]);
}

#[test]
fn another_order_is_another_set_of_the_same_claims() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = ScopeSet::default_workspace();
    let forward = database
        .record_claim_set(&scopes, &set_of(vec![label(), retries()]))
        .unwrap();
    let backward = database
        .record_claim_set(&scopes, &set_of(vec![retries(), label()]))
        .unwrap();
    assert_ne!(forward.id, backward.id);
    assert_eq!(forward.claims[0], backward.claims[1]);
    assert_eq!(forward.claims[1], backward.claims[0]);
    assert_eq!(counts(&scratch), [2, 2, 2, 4]);
}

#[test]
fn a_reopened_store_reads_the_same_set() {
    let scratch = Scratch::new();
    let record = scratch
        .open()
        .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![label()]))
        .unwrap();
    let reopened = scratch.open();
    assert_eq!(
        reopened
            .claim_set(&ScopeSet::default_workspace(), &record.id)
            .unwrap(),
        Some(record)
    );
}

#[test]
fn qualifiers_supports_and_every_literal_type_read_back_as_written() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut qualified = default_claim("enabled", LiteralKind::Boolean, "true", LABEL_ROW);
    qualified.conditions = BTreeMap::from([
        ("environment".to_owned(), "production".to_owned()),
        ("platform".to_owned(), "linux".to_owned()),
    ]);
    qualified.version = Validity::Bounded {
        start: Some("9.0.22".to_owned()),
        end: None,
    };
    qualified.world = Validity::Bounded {
        start: None,
        end: Some("2027-01-01T00:00:00.000Z".to_owned()),
    };
    qualified
        .supports
        .push(quoting("| label |", "block-label-cell"));
    let ratio = default_claim("ratio", LiteralKind::Decimal, "-0.50", LABEL_ROW);
    let set = set_of(vec![qualified, ratio, label(), retries()]);
    let record = database
        .record_claim_set(&ScopeSet::default_workspace(), &set)
        .unwrap();
    let read = database
        .claim_set(&ScopeSet::default_workspace(), &record.id)
        .unwrap()
        .unwrap();
    let mut claims: Vec<_> = read.claims.into_iter().map(|claim| claim.claim).collect();
    claims[0]
        .supports
        .sort_by_key(|support| support.block_id.clone());
    let mut expected = set.claims;
    expected[0]
        .supports
        .sort_by_key(|support| support.block_id.clone());
    assert_eq!(claims, expected);
}

#[test]
fn each_review_state_reads_back() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let scopes = ScopeSet::default_workspace();
    let record = database
        .record_claim_set(&scopes, &set_of(vec![label()]))
        .unwrap();
    for (name, state) in [
        ("accepted", ReviewState::Accepted),
        ("rejected", ReviewState::Rejected),
        ("flagged", ReviewState::Flagged),
        ("unreviewed", ReviewState::Unreviewed),
    ] {
        execute(
            &database,
            &format!("UPDATE claims SET review_state = '{name}'"),
        )
        .unwrap();
        let read = database.claim_set(&scopes, &record.id).unwrap().unwrap();
        assert_eq!(read.claims[0].review, state, "{name}");
    }
}

#[test]
fn an_unknown_or_unscoped_set_reads_as_none() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let record = database
        .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![label()]))
        .unwrap();
    let other = granted(&database, "other", "workspace/default/collection/other");
    assert_eq!(database.claim_set(&other, &record.id).unwrap(), None);
    let source = granted(
        &database,
        "source",
        "workspace/default/collection/graph/source/docs",
    );
    assert_eq!(database.claim_set(&source, &record.id).unwrap(), None);
    let collection = granted(
        &database,
        "collection",
        "workspace/default/collection/graph",
    );
    assert_eq!(
        database.claim_set(&collection, &record.id).unwrap(),
        Some(record)
    );
    let unknown = Digest::of(b"no such set");
    assert_eq!(
        database
            .claim_set(&ScopeSet::default_workspace(), &unknown)
            .unwrap(),
        None
    );
}

#[test]
fn a_collection_outside_the_scopes_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let other = granted(&database, "other", "workspace/default/collection/other");
    // A source alone does not cover its collection's claims.
    let source = granted(
        &database,
        "source",
        "workspace/default/collection/graph/source/docs",
    );
    for scopes in [other, source] {
        let error = database
            .record_claim_set(&scopes, &set_of(vec![label()]))
            .unwrap_err();
        assert!(matches!(error, Error::Unauthorized), "{error:?}");
    }
    assert_eq!(counts(&scratch), [0; 4]);
}

#[test]
fn a_revision_of_another_collection_or_none_is_refused_as_unknown() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut elsewhere = label();
    elsewhere.supports[0].revision_id = "rev-o".to_owned();
    let mut missing = label();
    missing.supports[0].revision_id = "rev-missing".to_owned();
    for (claim, revision) in [(elsewhere, "rev-o"), (missing, "rev-missing")] {
        let error = database
            .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![claim]))
            .unwrap_err();
        assert!(
            matches!(&error, Error::UnknownRevision { revision_id } if revision_id == revision),
            "{error:?}"
        );
    }
    assert_eq!(counts(&scratch), [0; 4]);
}

/// Each claim set a rule of form refuses, with a word of its reason.
fn malformed() -> Vec<(ClaimSet, &'static str)> {
    let mut cases = vec![
        (set_of(Vec::new()), "no claim"),
        (set_of(vec![label(), label()]), "twice"),
    ];
    let mut edit = |reason: &'static str, change: fn(&mut Claim)| {
        let mut claim = label();
        change(&mut claim);
        cases.push((set_of(vec![claim]), reason));
    };
    edit("support", |claim| claim.supports.clear());
    edit("twice", |claim| {
        claim.supports.push(claim.supports[0].clone());
    });
    edit("block", |claim| claim.supports[0].block_id.clear());
    edit("empty span", |claim| {
        claim.supports[0].span.end = claim.supports[0].span.start;
    });
    edit("subject name", |claim| claim.subject.name.clear());
    edit("extractor", |claim| claim.provenance.extractor.clear());
    edit("condition", |claim| {
        claim.conditions.insert(String::new(), "x".to_owned());
    });
    edit("bound", |claim| {
        claim.version = Validity::Bounded {
            start: Some(String::new()),
            end: None,
        };
    });
    edit("bound", |claim| {
        claim.world = Validity::Bounded {
            start: None,
            end: Some(String::new()),
        };
    });
    for (kind, lexeme) in [
        (LiteralKind::Boolean, "True"),
        (LiteralKind::Boolean, ""),
        (LiteralKind::Integer, ""),
        (LiteralKind::Integer, "-"),
        (LiteralKind::Integer, "3.0"),
        (LiteralKind::Integer, "+3"),
        (LiteralKind::Decimal, "0"),
        (LiteralKind::Decimal, ".5"),
        (LiteralKind::Decimal, "5."),
        (LiteralKind::Decimal, "1e3"),
        (LiteralKind::Decimal, "0.5.1"),
    ] {
        cases.push((
            set_of(vec![default_claim("label", kind, lexeme, LABEL_ROW)]),
            "lexeme",
        ));
    }
    cases
}

#[test]
fn a_claim_of_the_wrong_form_is_refused_and_nothing_is_recorded() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for (set, reason) in malformed() {
        let error = database
            .record_claim_set(&ScopeSet::default_workspace(), &set)
            .unwrap_err();
        assert!(
            matches!(&error, Error::Invalid(message) if message.contains(reason)),
            "{reason}: {error:?}"
        );
    }
    assert_eq!(counts(&scratch), [0; 4]);
}

/// Unlisted endpoint kinds must fail the authority's CHECK, leaving all claim
/// tables empty even when bypassing typed claim construction.
fn refuse_authority_kinds(cases: &[(&str, &str)]) {
    let scratch = Scratch::new();
    let database = scratch.open();
    for (subject, object) in cases {
        let sql = format!(
            "INSERT INTO claims (id, collection_id, subject_kind, subject_name,
             predicate, object_kind, object_name, conditions_json, version_known,
             world_known, extractor, profile_digest, support_count)
             VALUES ('raw', 'graph', '{subject}', 'scheduled-run', 'DEPENDS_ON',
             '{object}', 'documented-type', '{{}}', 0, 0, 'test', 'profile', 1)"
        );
        let error = execute(&database, &sql).unwrap_err();
        assert!(
            matches!(error, store::Error::Sqlite(SqliteError::SqliteFailure(code, _))
                if code.code == ErrorCode::ConstraintViolation
                    && code.extended_code == SQLITE_CONSTRAINT_CHECK),
            "{subject} -> {object}: {error:?}"
        );
        assert_eq!(counts(&scratch), [0; 4], "{subject} -> {object}");
    }
}

#[test]
fn an_instance_job_kind_is_refused_at_both_authority_endpoints() {
    assert_eq!(EntityKind::parse("Job"), None);
    refuse_authority_kinds(&[("Job", "Component"), ("Component", "Job")]);
}

#[test]
fn event_and_job_type_kinds_cannot_extend_depends_on_at_the_authority() {
    assert_eq!(EntityKind::parse("Event"), None);
    assert_eq!(EntityKind::parse("JobType"), None);
    refuse_authority_kinds(&[
        ("Event", "Component"),
        ("Concept", "JobType"),
        ("Component", "Event"),
        ("JobType", "Component"),
    ]);
}

#[test]
fn well_formed_lexemes_of_each_type_are_admitted() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let claims = [
        (LiteralKind::Text, ""),
        (LiteralKind::Boolean, "false"),
        (LiteralKind::Integer, "-12"),
        (LiteralKind::Integer, "007"),
        (LiteralKind::Decimal, "0.50"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (kind, lexeme))| default_claim(&format!("p{index}"), kind, lexeme, LABEL_ROW))
    .collect();
    let record = database
        .record_claim_set(&ScopeSet::default_workspace(), &set_of(claims))
        .unwrap();
    assert_eq!(record.claims.len(), 5);
}
