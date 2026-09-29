//! Stable backend collection names and publication reports.

use super::{
    names::{alias_name, collection_name},
    progress::Report,
};
use maestro_kernel::{chunk_set::ChunkSet, generation::Generation};

/// The names of a generation in the projection backend.
#[derive(Debug)]
pub(super) struct Names {
    /// Its generation ID.
    pub(super) generation: i64,
    /// Its physical collection.
    pub(super) collection: String,
    /// Its collection's alias.
    pub(super) alias: String,
}

impl Names {
    /// The names of `generation`.
    pub(super) fn of(generation: &Generation) -> Self {
        Self {
            generation: generation.id,
            collection: collection_name(generation),
            alias: alias_name(generation),
        }
    }
}

/// The report of a generation published under its names.
pub(super) fn report(
    set: &ChunkSet,
    generation: &Generation,
    names: &Names,
    points: u64,
    retired: Option<i64>,
) -> Report {
    Report {
        collection: set.collection_id.clone(),
        chunk_set: set.id.clone(),
        generation: generation.id,
        qdrant_collection: names.collection.clone(),
        alias: names.alias.clone(),
        points,
        embedding_profile: generation.embedding_profile.clone(),
        sparse_profile: generation.sparse_profile.clone(),
        retired,
    }
}
