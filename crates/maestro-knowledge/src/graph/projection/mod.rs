//! Public typed-edge and literal-fact projection ports and unpublished build writer.

pub mod health;
pub mod port;
#[cfg(feature = "engine")]
pub mod probe;

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
    reason = "engine-absent builds retain injectable readiness tests"
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
