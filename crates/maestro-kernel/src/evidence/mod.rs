//! Evidence (building block B7; docs/architecture/02 §6, plan D10): what a
//! search returns is text read from the authority, never text remembered
//! elsewhere.
//!
//! `Database::resolve` turns a chunk into an [`Excerpt`]: the exact bytes its
//! span covers in its revision's original Markdown, read from the artifact
//! store under the revision's `original_digest` and checked against it, with
//! the SHA-256 of that text, the span, the revision's `version` metadata and
//! what identifies them. A chunk's own digest hashes its prepared input,
//! context included, so it is never the excerpt's. Stored bytes that no
//! longer match their digest, a span past the end of the text and a span that
//! starts or ends inside a character are each refused with an [`Error`] of
//! their own.
//!
//! A [`Bundle`] is the search response contract, `maestro-evidence/1`: the
//! passages it cites, each with what identifies and cites it; whether each
//! route and the reranker ran, with the reason of one that could not; its
//! known gaps, its conflicts and its budget; and, apart from the evidence,
//! the trace of how each passage was found and ranked. Its JSON is strict: the
//! schema is checked, an unknown key is refused, a span never starts after it
//! ends, passage numbers start from 1 and are given once, though they may
//! skip, and each conflict and trace entry names a passage the bundle holds.
//! A bundle writes and reads back equal, its scores to the bit.

mod bundle;
mod error;
mod passage;
mod resolve;
#[cfg(test)]
mod tests;

pub use bundle::{Budget, Bundle, Conflict, RouteStatus, Schema, Trace};
pub use error::Error;
pub use passage::{Alternate, Passage, Span};
pub use resolve::Excerpt;
