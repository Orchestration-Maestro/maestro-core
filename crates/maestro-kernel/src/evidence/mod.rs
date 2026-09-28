//! Evidence (building block B7; docs/architecture/02 §6, plan D10): what a
//! search returns is text read from the authority, never text remembered
//! elsewhere.
//!
//! `Database::resolve` turns a chunk into an [`Excerpt`]: the exact bytes its
//! span covers in its revision's original Markdown, read from the artifact
//! store under the revision's `original_digest` and checked against it, with
//! the SHA-256 of that text, the span, the revision's `version` metadata and
//! what identifies them. A chunk's own digest hashes its prepared input,
//! context included, so it is never the excerpt's. It reads a chunk only if
//! the caller's scopes cover its document's source, and refuses any other as
//! it refuses a chunk that does not exist, so a refusal never reveals one.
//! Stored bytes that no longer match their digest, a span past the end of the
//! text and a span that starts or ends inside a character are each refused
//! with an [`Error`] of their own.
//!
//! A [`Bundle`] is the search response contract, `maestro-evidence/1`: the
//! passages it cites, each with what identifies and cites it; whether each
//! route and the reranker ran, with the reason of one that could not; its
//! known gaps, its conflicts and its budget; and, apart from the evidence,
//! the trace of how each passage was found and ranked. Its JSON is strict: the
//! schema is checked, and an unknown key or a route named twice is refused.
//! Its parts agree, both when it is written, which is refused before anything
//! is written, and when it is read: passage spans are valid, positive passage
//! numbers are unique, and evidence size is within the recorded counter and
//! request bounds. Unavailable routes give reasons; conflicts and traces refer
//! only to held passages, with valid scores; inventories are validated and
//! require a successful structured route. So a bundle that writes reads back
//! equal, its scores to the bit.

mod bundle;
mod error;
mod inventory;
mod lookup;
mod passage;
mod request_budget;
mod resolve;
#[cfg(test)]
mod tests;

pub use bundle::{Budget, Bundle, Conflict, RouteStatus, Schema, Trace};
pub use error::Error;
pub use inventory::{Inventory, InventoryCount, TRUNCATED_INVENTORY_GAP_PREFIX};
pub use lookup::{ChunkLocation, SectionLocation};
pub use passage::{Alternate, Passage, Span};
pub use request_budget::RequestBudget;
pub use resolve::Excerpt;
