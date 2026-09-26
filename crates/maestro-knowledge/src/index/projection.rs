//! What a publication works with.

use super::qdrant::Qdrant;
use maestro_kernel::{gateway::ModelCard, scope::ScopeSet, store::Database};

/// What a publication works with: the kernel it reads a chunk set from and
/// records the generation in, through the caller's scopes, the Qdrant that
/// holds the generation's collection, and the embedder of the generation's
/// dense vectors, its card and the model port that reaches it.
#[derive(Debug)]
pub struct Projection<'a, P> {
    /// The kernel.
    pub database: &'a Database,
    /// What the caller reads: the chunk set's collection, whole.
    pub scopes: &'a ScopeSet,
    /// The Qdrant server.
    pub qdrant: &'a Qdrant,
    /// The model port the embedder answers through.
    pub port: &'a P,
    /// The embedder's model card.
    pub card: &'a ModelCard,
}
