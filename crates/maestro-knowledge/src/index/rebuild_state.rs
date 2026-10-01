//! Snapshot types for one guarded generation replacement.

use super::progress::Progress;
use maestro_kernel::{
    chunk_set::{Chunk, ChunkSet},
    generation::Generation,
};

/// The published pointer and generation boundary frozen when a rebuild starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuildGuard {
    /// The published generation at admission, or none.
    pub expected_published: Option<i64>,
    /// Generations at or before this ID cannot be adopted by this rebuild.
    pub generation_watermark: i64,
}

/// Frozen database and backend state for one guarded replacement.
pub(super) struct RebuildState<'a> {
    /// Dimensions of the card's dense vectors.
    pub(super) dimensions: u64,
    /// Complete chunk set to rebuild.
    pub(super) set: ChunkSet,
    /// Embedding profile expected from the restored card.
    pub(super) expected_embedding: String,
    /// Chunks whose points the replacement must hold.
    pub(super) chunks: Vec<Chunk>,
    /// Published generation observed while admitting the replacement.
    pub(super) published: Option<Generation>,
    /// Generation named by journaled progress, if resuming.
    pub(super) resume_generation: Option<Generation>,
    /// Alias targets this replacement may reconcile.
    pub(super) allowed_aliases: [Option<String>; 3],
    /// Published pointer and generation watermark frozen by the job.
    pub(super) guard: RebuildGuard,
    /// Last progress event recorded by the job, if resuming.
    pub(super) resume: Option<&'a Progress>,
}

impl RebuildState<'_> {
    /// The generation ID observed as published during admission.
    pub(super) fn published_id(&self) -> Option<i64> {
        self.published.as_ref().map(|generation| generation.id)
    }
}
