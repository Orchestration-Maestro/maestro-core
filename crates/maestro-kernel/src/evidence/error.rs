//! Why the kernel refused to resolve a chunk.

use super::passage::Span;
use crate::{artifact::Digest, store};
use std::{error, fmt};

/// Why the kernel refused to resolve a chunk.
#[derive(Debug)]
pub enum Error {
    /// The chunk set holds no chunk of this id.
    UnknownChunk {
        /// The chunk set asked.
        chunk_set_id: String,
        /// The chunk asked for.
        chunk_id: String,
    },
    /// The bytes stored as the revision's original Markdown are not the ones
    /// it recorded: they no longer hash to its digest.
    DigestMismatch {
        /// The revision.
        revision_id: String,
        /// The digest it recorded.
        expected: Digest,
        /// The digest of the bytes found in its place.
        found: Digest,
    },
    /// The chunk's span reaches past the end of its revision's original
    /// Markdown.
    SpanOutOfRange {
        /// The revision.
        revision_id: String,
        /// The chunk's span.
        span: Span,
        /// How many bytes the original holds.
        length: usize,
    },
    /// The chunk's span starts or ends inside a character of its revision's
    /// original Markdown, so its bytes are not text.
    SpanOffBoundary {
        /// The revision.
        revision_id: String,
        /// The chunk's span.
        span: Span,
    },
    /// The kernel's database or artifact store refused; the message and the
    /// source are its own.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownChunk {
                chunk_set_id,
                chunk_id,
            } => write!(
                formatter,
                "the chunk set {chunk_set_id} holds no chunk {chunk_id}"
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
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(error) => error.source(),
            Self::UnknownChunk { .. }
            | Self::DigestMismatch { .. }
            | Self::SpanOutOfRange { .. }
            | Self::SpanOffBoundary { .. } => None,
        }
    }
}

impl From<store::Error> for Error {
    fn from(error: store::Error) -> Self {
        Self::Store(error)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}
