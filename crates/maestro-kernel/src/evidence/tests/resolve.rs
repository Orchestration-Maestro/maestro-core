//! Resolving a chunk: the exact bytes its span covers in its revision's
//! original Markdown, read from the artifact store and checked against the
//! revision's digest, with the digest of that text, the span, the version
//! and what identifies them.

use super::support::{ORIGINAL, SOURCE_REF, Scratch, chunk, metadata, span_of};
use crate::{
    artifact::{self, Digest},
    evidence::{Error, Excerpt, Span},
    store,
};
use serde_json::Value;
use std::{error, fs};

#[test]
fn a_chunk_resolves_to_the_exact_text_of_its_span_with_its_digest_span_and_version() {
    let scratch = Scratch::new();
    let database = scratch.open(metadata(Some(Value::from("2.1.0"))));
    let text = "The default port is 7006 — unless the installer finds it taken.\n";
    let span = span_of(text);
    assert_eq!(
        span.end,
        ORIGINAL.len(),
        "the span ends where the text does"
    );
    chunk(&database, "chunk-2", Some("sec-ports"), span);
    assert_eq!(
        database.resolve("set-a", "chunk-2").unwrap(),
        Excerpt {
            revision_id: "rev-a".to_owned(),
            document_id: "doc-a".to_owned(),
            section_id: Some("sec-ports".to_owned()),
            source_ref: SOURCE_REF.to_owned(),
            version: Some("2.1.0".to_owned()),
            span,
            digest: Digest::of(text.as_bytes()),
            text: text.to_owned(),
        }
    );
}

#[test]
fn a_revision_without_a_version_as_text_resolves_without_one() {
    for version in [None, Some(Value::Null), Some(Value::from(2))] {
        let scratch = Scratch::new();
        let database = scratch.open(metadata(version));
        chunk(&database, "chunk-1", None, span_of("The agent listens"));
        let excerpt = database.resolve("set-a", "chunk-1").unwrap();
        assert_eq!(excerpt.version, None);
        assert_eq!(excerpt.section_id, None);
        assert_eq!(excerpt.text, "The agent listens");
    }
}

#[test]
fn an_original_that_no_longer_matches_its_digest_is_refused_naming_both_digests() {
    let scratch = Scratch::new();
    let database = scratch.open(metadata(None));
    chunk(&database, "chunk-1", None, span_of("The agent listens"));
    let recorded = Digest::of(ORIGINAL.as_bytes());
    let tampered = ORIGINAL.replace("7005", "7007");
    fs::write(scratch.stored(&recorded), &tampered).unwrap();
    let error = database.resolve("set-a", "chunk-1").unwrap_err();
    assert!(
        matches!(
            &error,
            Error::DigestMismatch { revision_id, expected, found }
                if revision_id == "rev-a"
                    && *expected == recorded
                    && *found == Digest::of(tampered.as_bytes())
        ),
        "{error:?}"
    );
}

#[test]
fn an_original_that_is_gone_is_refused_as_the_store_reports_it() {
    let scratch = Scratch::new();
    let database = scratch.open(metadata(None));
    chunk(&database, "chunk-1", None, span_of("The agent listens"));
    let recorded = Digest::of(ORIGINAL.as_bytes());
    fs::remove_file(scratch.stored(&recorded)).unwrap();
    let error = database.resolve("set-a", "chunk-1").unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Store(store::Error::Artifact(artifact::Error::Missing(missing)))
                if *missing == recorded
        ),
        "{error:?}"
    );
}

#[test]
fn a_span_past_the_end_of_the_original_is_refused() {
    let length = ORIGINAL.len();
    let spans = [
        Span {
            start: length - 3,
            end: length + 1,
        },
        Span {
            start: length + 1,
            end: length + 2,
        },
    ];
    for past in spans {
        let scratch = Scratch::new();
        let database = scratch.open(metadata(None));
        chunk(&database, "chunk-1", None, past);
        let error = database.resolve("set-a", "chunk-1").unwrap_err();
        assert!(
            matches!(
                &error,
                Error::SpanOutOfRange { revision_id, span, length: bytes }
                    if revision_id == "rev-a" && *span == past && *bytes == length
            ),
            "{error:?}"
        );
    }
}

#[test]
fn a_span_that_starts_or_ends_inside_a_character_is_refused() {
    // The three bytes of the dash of the section `Ports`.
    let dash = ORIGINAL.find('—').unwrap();
    let spans = [
        Span {
            start: dash - 1,
            end: dash + 1,
        },
        Span {
            start: dash + 2,
            end: dash + 5,
        },
    ];
    for inside in spans {
        let scratch = Scratch::new();
        let database = scratch.open(metadata(None));
        chunk(&database, "chunk-1", None, inside);
        let error = database.resolve("set-a", "chunk-1").unwrap_err();
        assert!(
            matches!(
                &error,
                Error::SpanOffBoundary { revision_id, span }
                    if revision_id == "rev-a" && *span == inside
            ),
            "{error:?}"
        );
    }
}

#[test]
fn a_chunk_its_chunk_set_does_not_hold_is_refused_as_unknown() {
    let scratch = Scratch::new();
    let database = scratch.open(metadata(None));
    chunk(&database, "chunk-1", None, span_of("The agent listens"));
    for (set, id) in [("set-a", "chunk-9"), ("set-b", "chunk-1")] {
        let error = database.resolve(set, id).unwrap_err();
        assert!(
            matches!(
                &error,
                Error::UnknownChunk { chunk_set_id, chunk_id }
                    if chunk_set_id == set && chunk_id == id
            ),
            "{error:?}"
        );
    }
}

#[test]
fn every_refusal_says_what_went_wrong() {
    let span = Span { start: 3, end: 9 };
    let (expected, found) = (Digest::of(b"a"), Digest::of(b"b"));
    let refusals = [
        (
            Error::UnknownChunk {
                chunk_set_id: "set-a".to_owned(),
                chunk_id: "chunk-9".to_owned(),
            },
            vec!["chunk-9", "set-a"],
        ),
        (
            Error::DigestMismatch {
                revision_id: "rev-a".to_owned(),
                expected: expected.clone(),
                found: found.clone(),
            },
            vec!["rev-a", expected.as_str(), found.as_str()],
        ),
        (
            Error::SpanOutOfRange {
                revision_id: "rev-a".to_owned(),
                span,
                length: 5,
            },
            vec!["rev-a", "[3, 9)", "5 bytes"],
        ),
        (
            Error::SpanOffBoundary {
                revision_id: "rev-a".to_owned(),
                span,
            },
            vec!["rev-a", "[3, 9)", "inside a character"],
        ),
    ]
    .map(|(error, words)| {
        (
            error.to_string(),
            words,
            error::Error::source(&error).is_none(),
        )
    });
    for (message, words, sourceless) in refusals {
        assert!(sourceless, "{message}");
        for word in words {
            assert!(message.contains(word), "{word:?} in {message}");
        }
    }
    let store = Error::Store(store::Error::UnknownMigration("0099_later".to_owned()));
    assert!(store.to_string().contains("0099_later"), "{store}");
    assert!(error::Error::source(&store).is_none());
    let sqlite = Error::from(rusqlite::Error::InvalidQuery);
    assert!(error::Error::source(&sqlite).is_some(), "{sqlite}");
}
