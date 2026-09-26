//! Preparing revisions for search (docs/architecture/01 §6 and §7): a
//! collection's eligible revisions deduplicated, then cut into chunks
//! budgeted in the tokens of the embedder that will read them, counted
//! through the model router's `/tokenize` for that embedder's model card,
//! with no machine path and no process per count (ADR-0008).
//!
//! [`RouterTokenizer`] is maestro-canonicalization's `TokenCounter` for one
//! embedder's card, over any model port. The native counter stays the
//! reference: a router tokenizer exists only once the router gave every
//! parity fixture of the native profile the native counter's ordered IDs
//! (FR-S1-003).
//!
//! [`prepare`] reads a collection's eligible revisions, of each document its
//! latest revision when the quality gate lets it through
//! ([`quality::eligible`](crate::quality::eligible)), and records a chunk
//! set of them (FR-S1-003). A document whose latest revision is held back,
//! failed or not decided yet is left out, with no older revision in its
//! place, and named with why. Exact duplicates are prepared once, each place
//! they occur kept as an occurrence; near duplicates are grouped with their
//! confirmed Jaccard, never deleted; and each revision's chunks are cut by
//! maestro-canonicalization's chunker under its profile
//! `mapped-structural-chunks/2`, each with its exact prepared input stored as
//! the artifact its digest names. A revision the chunker refuses, such as
//! one with a unit that cannot fit 700 tokens with its context, gets no
//! chunk, and its refusal, which names the unit, is recorded with the chunk
//! set. The chunk set completes with its manifest, `maestro-chunk-set/1`,
//! only once every eligible revision is settled, so interrupted work never
//! reads as complete; a rerun resumes it.
//!
//! A preparation is a library operation, run as a leased job: its caller
//! submits the job with the chunk set it will build, [`chunk_set_id`], as its
//! frozen input, holds its lease, and [`prepare_observed`] shows it the report
//! after each batch, which the job journals as its progress.

mod bridge;
mod chunking;
mod collection;
mod counter;
mod error;
mod exact;
mod failure;
mod left_out;
mod manifest;
mod near;
mod parity;
mod report;
mod router_tokenizer;
#[cfg(test)]
mod tests;

pub use collection::{chunk_set_id, prepare, prepare_observed};
pub use error::TokenizerError;
pub use failure::Error;
pub use report::{Ineligibility, LeftOut, Refusal, Report};
pub use router_tokenizer::RouterTokenizer;
