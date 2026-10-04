//! Why the kernel refused to admit or read claims.

use super::projection_binding::{InputMismatchKind, PROJECTION_REBUILD_REPAIR};
use crate::{artifact::Digest, evidence::Span, job, store};
use std::{error, fmt};

/// Why the kernel refused to admit or read claims. A refused write records
/// nothing.
#[derive(Debug)]
pub enum Error {
    /// The caller's scopes do not cover the collection.
    Unauthorized,
    /// The claims break a rule of their own form: the reason says which.
    Invalid(String),
    /// The collection holds no revision of this id the caller may read:
    /// whether one exists outside the caller's scopes is never said.
    UnknownRevision {
        /// The revision a support names.
        revision_id: String,
    },
    /// The revision failed, or its quality disposition holds it back or is
    /// not given yet: it supports nothing.
    IneligibleRevision {
        /// The revision a support names.
        revision_id: String,
    },
    /// The bytes stored as the revision's original Markdown no longer hash
    /// to the digest it recorded.
    DigestMismatch {
        /// The revision.
        revision_id: String,
        /// The digest it recorded.
        expected: Digest,
        /// The digest of the bytes found in its place.
        found: Digest,
    },
    /// A support's span reaches past the end of its revision's original
    /// Markdown.
    SpanOutOfRange {
        /// The revision.
        revision_id: String,
        /// The support's span.
        span: Span,
        /// How many bytes the original holds.
        length: usize,
    },
    /// A support's span starts or ends inside a character.
    SpanOffBoundary {
        /// The revision.
        revision_id: String,
        /// The support's span.
        span: Span,
    },
    /// The bytes of a support's span do not hash to its quote digest.
    QuoteMismatch {
        /// The revision.
        revision_id: String,
        /// The support's span.
        span: Span,
        /// The support's quote digest.
        expected: Digest,
        /// The digest of the bytes of the span.
        found: Digest,
    },
    /// The caller names an unknown build.
    UnknownBuild(ulid::Ulid),
    /// The build's required batches are not all recorded.
    Unfinished {
        /// Number of batches recorded.
        recorded: usize,
        /// Number of batches required.
        expected: usize,
    },
    /// The build budget would be exceeded.
    OverBudget {
        /// Configured limit.
        limit: usize,
        /// Required total.
        needed: usize,
    },
    /// A durable graph build conflicts with its frozen plan or receipt.
    Conflict(String),
    /// A legacy projection lacks the required durable input format.
    ProjectionInputMismatch(InputMismatchKind),
    /// The validator refused the proposed resolution snapshot before persistence.
    ResolutionRejected,
    /// A lease operation failed.
    Job(job::Error),
    /// The kernel's database or artifact store refused.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => formatter.write_str("the collection is not in this scope"),
            Self::Invalid(reason) => write!(formatter, "invalid claim: {reason}"),
            Self::UnknownRevision { revision_id } => {
                write!(formatter, "the collection holds no revision {revision_id}")
            }
            Self::IneligibleRevision { revision_id } => write!(
                formatter,
                "the revision {revision_id} is not eligible to support a claim"
            ),
            Self::DigestMismatch {
                revision_id,
                expected,
                found,
            } => write!(
                formatter,
                "the original Markdown of the revision {revision_id} is not the one it recorded: \
                 it recorded sha256:{}, and the stored bytes hash to sha256:{}",
                expected.as_str(),
                found.as_str()
            ),
            Self::SpanOutOfRange {
                revision_id,
                span,
                length,
            } => write!(
                formatter,
                "the span {span} reaches past the {length} bytes of the original Markdown of \
                 the revision {revision_id}"
            ),
            Self::SpanOffBoundary { revision_id, span } => write!(
                formatter,
                "the span {span} starts or ends inside a character of the original Markdown \
                 of the revision {revision_id}"
            ),
            Self::QuoteMismatch {
                revision_id,
                span,
                expected,
                found,
            } => write!(
                formatter,
                "the span {span} of the revision {revision_id} hashes to sha256:{}, not to the \
                 quote digest sha256:{}",
                found.as_str(),
                expected.as_str()
            ),
            Self::UnknownBuild(id) => write!(formatter, "unknown graph build {id}"),
            Self::Unfinished { recorded, expected } => write!(
                formatter,
                "graph build has {recorded} of {expected} batches"
            ),
            Self::OverBudget { limit, needed } => {
                write!(formatter, "graph build budget {limit} exceeded by {needed}")
            }
            Self::Conflict(reason) => write!(formatter, "graph build conflict: {reason}"),
            Self::ProjectionInputMismatch(kind) => write!(
                formatter,
                "graph input mismatch ({kind}); {PROJECTION_REBUILD_REPAIR}"
            ),
            Self::ResolutionRejected => formatter.write_str("resolution snapshot rejected"),
            Self::Job(error) => fmt::Display::fmt(error, formatter),
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Job(error) => Some(error),
            Self::Unauthorized
            | Self::ProjectionInputMismatch(_)
            | Self::ResolutionRejected
            | Self::Invalid(_)
            | Self::UnknownRevision { .. }
            | Self::IneligibleRevision { .. }
            | Self::DigestMismatch { .. }
            | Self::SpanOutOfRange { .. }
            | Self::SpanOffBoundary { .. }
            | Self::QuoteMismatch { .. }
            | Self::UnknownBuild(_)
            | Self::Unfinished { .. }
            | Self::OverBudget { .. }
            | Self::Conflict(_) => None,
        }
    }
}

impl From<store::Error> for Error {
    fn from(error: store::Error) -> Self {
        Self::Store(error)
    }
}

impl From<job::Error> for Error {
    fn from(error: job::Error) -> Self {
        Self::Job(error)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}
