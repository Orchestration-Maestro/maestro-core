//! The knowledge graph's authority (specs/002-knowledge-graph, FR-S2-002 and
//! FR-S2-003): immutable claims, each supported by exact bytes of eligible
//! revisions, admitted in frozen, ordered claim sets.
//!
//! [`Database::record_claim_set`](crate::store::Database::record_claim_set)
//! and leased graph batches share the same claim admission path. It refuses
//! a collection the caller's scopes do not cover, and a revision of another
//! collection as one that does not exist.
//! It verifies every support itself, independently of whoever located it:
//! the revision belongs to the collection and is eligible (its document's
//! latest revision in record order, not failed, its disposition accepted),
//! its stored original still hashes to its digest, the span is nonempty,
//! within it and on character boundaries, and its bytes hash to the quote
//! digest. Then it records the whole set in one
//! write, or nothing. A claim's id and a set's are the SHA-256 of their
//! content, so a replay returns the recorded set and records nothing more.
//! Every claim is admitted unreviewed: a valid quote is not semantic truth.
//! Nothing recorded is replaced, changed or deleted, but a claim's review
//! state. A graph build records accepted claims and bounded rejection receipts
//! with its leased journal checkpoints, freezes a set only after all batches,
//! and attaches that set once to an unpublished generation.

mod attachment;
mod build;
mod build_read;
mod build_types;
mod error;
mod quote;
mod read;
mod resolve;
#[cfg(test)]
mod tests;
mod types;
mod write;

pub use build_types::{
    Batch, BatchReceipt, Budget, BuildPlan, BuildRecord, GraphAttachment, Rejection,
};
pub use error::Error;
pub use types::{
    Claim, ClaimRecord, ClaimSet, ClaimSetRecord, EntityName, Literal, LiteralKind, Object,
    Provenance, ReviewState, Support, Validity,
};

pub use crate::vocabulary::{EntityKind, Predicate};

pub use resolve::{
    Decision, DecisionKind, Endpoint, Mention, ResolutionInput, ResolutionSnapshot, ReviewRecord,
};
