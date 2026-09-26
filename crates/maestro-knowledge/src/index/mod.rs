//! The search projection in Qdrant (docs/architecture/01 §8 and §9; plan D9;
//! ADR-0002 and ADR-0003): each generation of a collection is one Qdrant
//! collection, `maestro-<collection>-g<n>`, behind the collection's alias,
//! `maestro-<collection>`. The kernel stays the authority: a generation is
//! built from one complete chunk set, and rebuilt from it at will.
//!
//! [`Projection::publish`] builds a generation of a chunk set, or resumes
//! the one it left building. Each chunk is one point, whose ID is the
//! `UUIDv5` of the chunk's ID, so the same chunk set gives the same point
//! IDs and writing a point again changes nothing. Both of its vectors come
//! from the chunk's exact prepared input, the text its chunk set counted:
//!
//! - **Dense**, named `dense`, of the embedder card's dimensions and
//!   compared by cosine. The embedder embeds a batch through the model port,
//!   in free room, so indexing never unloads another model (FR-S1-015a),
//!   within a deadline, and its vectors are checked before anything holds
//!   them: one per input, each of the card's dimensions, every value finite
//!   and its norm not zero ([`Refusal`]). The generation records their
//!   profile, `dense/1:sha256:<the card's digest>`: the card pins the model
//!   file and the router's build, and version 1 embeds a prepared input as
//!   it is, with no template around it.
//! - **Sparse**, named `bm25`: T040's analyzer, profile `bm25-en-fr/1`
//!   ([`lexical`](crate::lexical)), weighs each input's terms with BM25's
//!   term-frequency part against the average term count of every chunk of
//!   the set, which a first pass counts, and Qdrant multiplies each weight by
//!   the term's IDF (`modifier: idf`). Qdrant neither stores nor checks the
//!   analyzer, so the generation records its profile, and a query is
//!   analysed with the profile of the generation it searches.
//!
//! A point's payload carries the chunk's and its revision's IDs, its
//! section's heading path, its scope tags, and its revision's version and
//! source kind. Its scope tags are the scope of every source its content
//! occurs in, its document's and its occurrences', with every scope above
//! each: a search shows a caller the points one of whose tags is a scope
//! granted to it.
//!
//! Points are written in batches, each shown to the caller once Qdrant has
//! applied it: a job journals it as a step of its progress (T016), and a
//! rerun given the last step resumes after the chunks it counts. An
//! embedder that refuses, has no free room or gives vectors a check
//! refuses, a Qdrant that fails and a kernel that fails each stop the build
//! and leave the generation building, for a rerun to resume. Once every
//! point is written, the collection is checked: its vectors, its count and
//! the point of every chunk. Only a generation that passes is verified, the
//! alias moved to its collection in one action, and the generation
//! published, retiring the one before it, whose collection stays for a
//! rollback. A generation that fails a check fails for good, and the alias
//! stays where it was.
//!
//! [`Qdrant`] talks to Qdrant's gRPC API through its official Rust client,
//! `qdrant-client` 1.19 (ADR-0020).

mod batches;
mod dense;
mod error;
mod point;
mod progress;
mod projection;
mod provenance;
mod publish;
mod qdrant;
mod sparse;
#[cfg(test)]
mod tests;
mod verify;

pub use dense::{Failure, Refusal};
pub use error::{Error, Unverified};
pub use progress::{Progress, Report};
pub use projection::Projection;
pub use qdrant::{Qdrant, QdrantError};
