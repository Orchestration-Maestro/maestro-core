//! Preparing revisions for search (docs/architecture/01 §7): their chunks are
//! budgeted in the tokens of the embedder that will read them, counted
//! through the model router's `/tokenize` for that embedder's model card,
//! with no machine path and no process per count (ADR-0008).
//!
//! [`RouterTokenizer`] is maestro-canonicalization's `TokenCounter` for one
//! embedder's card, over any model port. The native counter stays the
//! reference: a router tokenizer exists only once the router gave every
//! parity fixture of the native profile the native counter's ordered IDs
//! (FR-S1-003).

mod bridge;
mod error;
mod parity;
mod router_tokenizer;
#[cfg(test)]
mod tests;

pub use error::TokenizerError;
pub use router_tokenizer::RouterTokenizer;
