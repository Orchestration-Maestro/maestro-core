//! What a publication works with.

use super::qdrant::Qdrant;
use maestro_kernel::{gateway::ModelCard, scope::ScopeSet, store::Database};
use std::num::NonZeroUsize;

/// What a publication works with: the kernel it reads a chunk set from and
/// records the generation in, through the caller's scopes, the Qdrant that
/// holds the generation's collection, and the embedder of the generation's
/// dense vectors, its card and the model port that reaches it.
#[derive(Debug)]
pub struct Projection<'a, P, R = Qdrant> {
    /// The kernel.
    pub database: &'a Database,
    /// What the caller reads: the chunk set's collection, whole.
    pub scopes: &'a ScopeSet,
    /// The retrieval projection backend.
    pub projection: &'a R,
    /// The model port the embedder answers through.
    pub port: &'a P,
    /// The embedder's model card.
    pub card: &'a ModelCard,
}

/// A projection using a test-selected batch size.
#[derive(Debug)]
pub struct ProjectionWithBatchSize<'a, P, R = Qdrant> {
    /// The projection to publish.
    pub(super) projection: Projection<'a, P, R>,
    /// The number of chunks to write in each batch.
    pub(super) batch_size: NonZeroUsize,
}

impl<'a, P, R> Projection<'a, P, R> {
    /// Gives a test a smaller batch size to reach later batches with a small
    /// fixture. Production `publish` methods use the batch size of 64.
    #[must_use]
    pub fn with_batch_size(self, batch_size: NonZeroUsize) -> ProjectionWithBatchSize<'a, P, R> {
        ProjectionWithBatchSize {
            projection: self,
            batch_size,
        }
    }
}
