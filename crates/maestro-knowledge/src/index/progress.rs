//! What a publication shows its caller after each batch, which a job journals
//! as its progress and a rerun resumes from, and what it reports once the
//! generation is published.

use serde::{Deserialize, Serialize};

/// A publication's progress once a batch is written: the generation it
/// builds, and how many chunks of its chunk set, in their order, its
/// collection holds. A job journals it as a step (T016), and a rerun given
/// the last step journaled resumes after those chunks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    /// The generation built.
    pub generation: i64,
    /// How many chunks of the set, from its first in record order, its
    /// collection holds.
    pub indexed: u64,
    /// How many chunks the set holds.
    pub chunks: u64,
    /// The average term count of the set's prepared inputs, which every
    /// sparse vector of the generation is weighed against.
    pub average_length: f64,
}

/// What a publication reports once its generation is published, as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// The collection published.
    pub collection: String,
    /// The chunk set its generation is built from.
    pub chunk_set: String,
    /// The generation published.
    pub generation: i64,
    /// The Qdrant collection that holds it, `maestro-<collection>-g<n>`.
    pub qdrant_collection: String,
    /// The alias that now points at it, `maestro-<collection>`.
    pub alias: String,
    /// The points its check counted: one per chunk of the set.
    pub points: u64,
    /// The profile of its dense vectors.
    pub embedding_profile: String,
    /// The profile of its sparse vectors, which a query is analysed with.
    pub sparse_profile: String,
    /// The generation it retired, the one published before it, if any.
    pub retired: Option<i64>,
}
