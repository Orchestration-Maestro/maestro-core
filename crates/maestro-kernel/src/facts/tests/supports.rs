//! Verifying supports: the kernel reads the quoted bytes from the revision's
//! original Markdown and checks them against its digest, the span and the
//! quote digest, admits only an eligible revision, and rolls a whole set
//! back when one support fails.

use super::support::{
    LABEL_ROW, ORIGINAL, Scratch, counts, execute, label, quoting, retries, revise, set_of,
};
use crate::{
    artifact::Digest,
    document::{Outcome, RevisionStatus},
    evidence::Span,
    facts::Error,
    scope::ScopeSet,
    store,
};
use std::{error::Error as _, fs};

#[test]
fn a_span_past_the_end_of_the_original_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut claim = label();
    claim.supports[0].span = Span {
        start: ORIGINAL.len() - 1,
        end: ORIGINAL.len() + 1,
    };
    let error = database
        .record_claim_set(
            &ScopeSet::default_workspace(),
            &set_of(vec![retries(), claim]),
        )
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::SpanOutOfRange { revision_id, span, length }
                if revision_id == "rev-a" && span.end == ORIGINAL.len() + 1
                    && *length == ORIGINAL.len()
        ),
        "{error:?}"
    );
    assert_eq!(counts(&scratch), [0; 4]);
}

#[test]
fn a_span_that_splits_a_character_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let accent = ORIGINAL.find('é').unwrap();
    for span in [
        Span {
            start: accent + 1,
            end: accent + 2,
        },
        Span {
            start: accent - 1,
            end: accent + 1,
        },
    ] {
        let mut claim = label();
        claim.supports[0].span = span;
        let error = database
            .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![claim]))
            .unwrap_err();
        assert!(
            matches!(&error, Error::SpanOffBoundary { revision_id, span: refused }
                if revision_id == "rev-a" && *refused == span),
            "{error:?}"
        );
    }
    assert_eq!(counts(&scratch), [0; 4]);
}

#[test]
fn a_quote_digest_that_is_not_the_digest_of_the_span_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut claim = label();
    claim.supports[0].quote_digest = Digest::of(b"| label | text | cafe |\n");
    let error = database
        .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![claim]))
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::QuoteMismatch { revision_id, expected, found, .. }
                if revision_id == "rev-a"
                    && *expected == Digest::of(b"| label | text | cafe |\n")
                    && *found == Digest::of(LABEL_ROW.as_bytes())
        ),
        "{error:?}"
    );
    assert_eq!(counts(&scratch), [0; 4]);
}

#[test]
fn an_original_that_no_longer_matches_its_digest_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let recorded = Digest::of(ORIGINAL.as_bytes());
    let tampered = ORIGINAL.replace("café", "cafe");
    fs::write(scratch.stored(&recorded), &tampered).unwrap();
    let error = database
        .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![label()]))
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::DigestMismatch { revision_id, expected, found }
                if revision_id == "rev-a" && *expected == recorded
                    && *found == Digest::of(tampered.as_bytes())
        ),
        "{error:?}"
    );
    assert_eq!(counts(&scratch), [0; 4]);
}

#[test]
fn a_missing_original_is_a_store_error() {
    let scratch = Scratch::new();
    let database = scratch.open();
    fs::remove_file(scratch.stored(&Digest::of(ORIGINAL.as_bytes()))).unwrap();
    let error = database
        .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![label()]))
        .unwrap_err();
    assert!(matches!(error, Error::Store(_)), "{error:?}");
}

#[test]
fn a_revision_that_is_failed_held_back_or_undecided_supports_nothing() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for (id, status, outcome) in [
        (
            "rev-failed",
            RevisionStatus::Failed,
            Some(Outcome::Accepted),
        ),
        (
            "rev-held",
            RevisionStatus::Valid,
            Some(Outcome::Quarantined),
        ),
        ("rev-undecided", RevisionStatus::Valid, None),
    ] {
        revise(&database, "doc-a", id, status, outcome);
        let mut claim = label();
        claim.supports[0].revision_id = id.to_owned();
        let error = database
            .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![claim]))
            .unwrap_err();
        assert!(
            matches!(&error, Error::IneligibleRevision { revision_id } if revision_id == id),
            "{error:?}"
        );
    }
    assert_eq!(counts(&scratch), [0; 4]);
}

#[test]
fn a_revision_supports_nothing_once_its_document_has_a_newer_one() {
    for (status, outcome) in [
        (RevisionStatus::Valid, Some(Outcome::Quarantined)),
        (RevisionStatus::Valid, Some(Outcome::Excluded)),
        (RevisionStatus::Valid, None),
        (RevisionStatus::Failed, Some(Outcome::Accepted)),
        (RevisionStatus::Valid, Some(Outcome::Accepted)),
    ] {
        let scratch = Scratch::new();
        let database = scratch.open();
        revise(&database, "doc-a", "rev-a2", status, outcome);
        let error = database
            .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![label()]))
            .unwrap_err();
        assert!(
            matches!(&error, Error::IneligibleRevision { revision_id } if revision_id == "rev-a"),
            "{outcome:?}: {error:?}"
        );
        assert_eq!(counts(&scratch), [0; 4]);
    }
}

#[test]
fn a_revision_accepted_with_warnings_supports_a_claim() {
    let scratch = Scratch::new();
    let database = scratch.open();
    revise(
        &database,
        "doc-a",
        "rev-warned",
        RevisionStatus::ValidWithWarnings,
        Some(Outcome::AcceptedWithWarnings),
    );
    let mut claim = label();
    claim.supports[0].revision_id = "rev-warned".to_owned();
    database
        .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![claim]))
        .unwrap();
    assert_eq!(counts(&scratch), [1, 1, 1, 1]);
}

#[test]
fn a_support_that_fails_inside_the_write_rolls_the_whole_set_back() {
    let scratch = Scratch::new();
    let database = scratch.open();
    execute(
        &database,
        "CREATE TRIGGER forced_failure BEFORE INSERT ON claim_supports
         WHEN NEW.block_id = 'block-refused'
         BEGIN SELECT RAISE(ABORT, 'forced support failure'); END",
    )
    .unwrap();
    let mut refused = retries();
    refused.supports[0].block_id = "block-refused".to_owned();
    let error = database
        .record_claim_set(
            &ScopeSet::default_workspace(),
            &set_of(vec![label(), refused]),
        )
        .unwrap_err();
    assert!(
        matches!(&error, Error::Store(_))
            && format!("{error:?}").contains("forced support failure"),
        "{error:?}"
    );
    assert_eq!(counts(&scratch), [0; 4]);
}

#[test]
fn a_support_of_several_rows_and_blocks_is_verified_on_its_own_bytes() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut claim = label();
    claim.supports.push(quoting(ORIGINAL, "block-document"));
    let record = database
        .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![claim]))
        .unwrap();
    assert_eq!(record.claims[0].claim.supports.len(), 2);
}

#[test]
fn each_refusal_says_why_and_only_a_store_error_has_a_source() {
    let span = Span { start: 1, end: 3 };
    let digest = Digest::of(b"a");
    let other = Digest::of(b"b");
    let cases = [
        (Error::Unauthorized, "the collection is not in this scope"),
        (
            Error::Invalid("no claim".to_owned()),
            "invalid claim: no claim",
        ),
        (
            Error::UnknownRevision {
                revision_id: "rev-x".to_owned(),
            },
            "the collection holds no revision rev-x",
        ),
        (
            Error::IneligibleRevision {
                revision_id: "rev-x".to_owned(),
            },
            "the revision rev-x is not eligible to support a claim",
        ),
        (
            Error::DigestMismatch {
                revision_id: "rev-x".to_owned(),
                expected: digest.clone(),
                found: other.clone(),
            },
            &format!(
                "the original Markdown of the revision rev-x is not the one it recorded: it \
                 recorded sha256:{}, and the stored bytes hash to sha256:{}",
                digest.as_str(),
                other.as_str()
            ),
        ),
        (
            Error::SpanOutOfRange {
                revision_id: "rev-x".to_owned(),
                span,
                length: 2,
            },
            "the span [1, 3) reaches past the 2 bytes of the original Markdown of the revision \
             rev-x",
        ),
        (
            Error::SpanOffBoundary {
                revision_id: "rev-x".to_owned(),
                span,
            },
            "the span [1, 3) starts or ends inside a character of the original Markdown of the \
             revision rev-x",
        ),
        (
            Error::QuoteMismatch {
                revision_id: "rev-x".to_owned(),
                span,
                expected: digest.clone(),
                found: other.clone(),
            },
            &format!(
                "the span [1, 3) of the revision rev-x hashes to sha256:{}, not to the quote \
                 digest sha256:{}",
                other.as_str(),
                digest.as_str()
            ),
        ),
    ];
    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
        assert!(error.source().is_none(), "{error:?}");
    }
    let store = Error::from(store::Error::from(rusqlite::Error::InvalidQuery));
    assert_eq!(
        store.to_string(),
        store::Error::from(rusqlite::Error::InvalidQuery).to_string()
    );
    assert!(store.source().is_some());
}
