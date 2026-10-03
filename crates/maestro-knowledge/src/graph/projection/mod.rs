//! Public typed-edge and literal-fact projection ports and unpublished build writer.

pub mod port;

#[cfg(not(feature = "engine"))]
mod absent;
#[cfg(feature = "engine")]
mod access;
mod adapter;
mod build;
mod cancellation;
pub mod cleanup;
mod content;
mod handle;
pub use cancellation::ProjectionCancellation;
#[cfg(feature = "engine")]
mod engine;
mod lifecycle;
mod operations;
#[expect(
    dead_code,
    reason = "the E09 health adapter consumes this readiness port"
)]
mod receipts;
mod schema;
mod settings;
pub use build::{ProjectionBuild, PublishedProjection};
pub use handle::ProjectionHandle;
pub use lifecycle::{ProjectionFactory, ProjectionProducer};
pub use settings::{EngineSettings, ProjectionEngine};
pub use writer::{BuildVerification, CatalogRelationVocabulary};
pub(crate) mod writer;
pub use port::{
    EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope, TypedEdgeProjection,
};
#[cfg(test)]
mod tests;
