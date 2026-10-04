//! How `knowledge graph build` classifies the kernel's refusals of a claim
//! set: a refused input exits 2, a kernel or integrity failure exits 1.

use super::failure::claim_failure;
use crate::failure::Failure;
use maestro_kernel::{artifact::Digest, evidence::Span, facts::Error, job};
use std::process::ExitCode;
use ulid::Ulid;

/// A revision id the errors name.
const REVISION: &str = "rev-graph";

/// Whether the kernel's `error` makes the build fail, exit 1, rather than
/// refuse, exit 2.
fn fails(error: &Error) -> bool {
    matches!(claim_failure(error), Failure::Failed(_))
}

#[test]
fn integrity_and_store_errors_fail() {
    let span = Span { start: 0, end: 1 };
    let (expected, found) = (Digest::of(b"recorded"), Digest::of(b"stored"));
    for error in [
        Error::from(rusqlite::Error::QueryReturnedNoRows),
        Error::DigestMismatch {
            revision_id: REVISION.to_owned(),
            expected: expected.clone(),
            found: found.clone(),
        },
        Error::SpanOutOfRange {
            revision_id: REVISION.to_owned(),
            span,
            length: 0,
        },
        Error::SpanOffBoundary {
            revision_id: REVISION.to_owned(),
            span,
        },
        Error::QuoteMismatch {
            revision_id: REVISION.to_owned(),
            span,
            expected,
            found,
        },
    ] {
        assert!(fails(&error), "{error:?}");
    }
}

#[test]
fn refused_inputs_are_refused() {
    for error in [
        Error::Unauthorized,
        Error::Invalid("a claim set holds no claim".to_owned()),
        Error::UnknownRevision {
            revision_id: REVISION.to_owned(),
        },
        Error::IneligibleRevision {
            revision_id: REVISION.to_owned(),
        },
    ] {
        assert!(!fails(&error), "{error:?}");
    }
}

#[test]
fn object_documents_keep_literal_bytes_and_name_entity_endpoints() {
    use super::build::ObjectDocument;
    use maestro_kernel::facts::{EntityKind, EntityName, Literal, LiteralKind, Object};

    for (object, expected) in [
        (
            Object::Literal(Literal {
                kind: LiteralKind::Decimal,
                lexeme: "0.50".to_owned(),
            }),
            r#"{"type":"decimal","lexeme":"0.50"}"#,
        ),
        (
            Object::Entity(EntityName {
                kind: EntityKind::Api,
                name: "GET /status".to_owned(),
            }),
            r#"{"kind":"API","name":"GET /status"}"#,
        ),
    ] {
        assert_eq!(
            serde_json::to_string(&ObjectDocument::from(&object)).unwrap(),
            expected
        );
    }
}

#[test]
fn durable_build_refusals_exit_two_and_lease_failures_exit_one() {
    for error in [
        Error::UnknownBuild(Ulid::nil()),
        Error::Unfinished {
            recorded: 1,
            expected: 2,
        },
        Error::OverBudget {
            limit: 1,
            needed: 2,
        },
        Error::Conflict("batch replay differs".into()),
    ] {
        assert_eq!(claim_failure(&error).code(), ExitCode::from(2), "{error:?}");
    }
    let error = Error::Job(job::Error::Time);
    assert_eq!(claim_failure(&error).code(), ExitCode::from(1));
}

pub(super) mod cleanup_support;
mod extractor_selection;
pub(super) mod runner_tests;
