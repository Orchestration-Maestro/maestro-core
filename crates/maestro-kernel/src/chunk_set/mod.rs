//! Chunk sets (docs/architecture/01 §7 and §11; plan D6): the chunks of a
//! collection's eligible revisions under one chunk profile and one token
//! counter, which a search generation is built from (D9).
//!
//! Its caller names a chunk set by an id it derives from everything that
//! makes the chunks: the collection, the profile, the counter's contract ID
//! and the revisions it chunks. So a new profile, counter or set of revisions
//! begins a new chunk set, and none is ever overwritten. A chunk set begins
//! `building`, then moves only to `complete`, with the manifest artifact that
//! describes it, or to `failed`; both are final. A rerun begins the same set
//! again and gets it as it is: it resumes one that is building, and finds
//! one that is complete or failed.
//!
//! A chunk is one passage of a revision: the chunker's id, its section, the
//! artifact of its exact prepared input, its token count and the span of its
//! revision's original Markdown it covers. The chunks of a revision are
//! recorded at once, into a building set only, each pinning its prepared
//! input in the same write, so what the embedder reads is what was counted,
//! and an interrupted preparation leaves each revision chunked whole or not
//! at all.
//!
//! The migration `0007_chunk_sets` holds every writer to the same rules, not
//! only these calls: a set is inserted building, moves only from building,
//! keeps its identity and is never replaced nor deleted, and a chunk enters
//! only a building set, for a revision of the set's collection, and never
//! changes.
//!
//! A chunk set has its collection's scope, and a chunk its revision's
//! source's: every reader takes the caller's
//! [`ScopeSet`](crate::scope::ScopeSet), which must cover them.

mod chunk;
mod error;
mod record;
mod state;
#[cfg(test)]
mod tests;

pub use chunk::Chunk;
pub use error::Error;
pub use record::{ChunkSet, NewChunkSet};
pub use state::ChunkSetState;
