//! Public typed-edge and literal-fact projection ports and unpublished build writer.

pub mod health;
pub mod port;
#[cfg(feature = "engine")]
pub mod probe;

#[cfg(feature = "engine")]
mod access;
#[cfg(feature = "engine")]
mod adapter;
mod binding;
mod build;
mod cancellation;
#[cfg(any(feature = "engine", test))]
mod checkpoint;
pub mod cleanup;
#[cfg(feature = "engine")]
mod configuration;
mod content;
#[cfg(feature = "engine")]
mod handle;
mod import;
pub use cancellation::ProjectionCancellation;
#[cfg(feature = "engine")]
mod engine;
#[cfg(feature = "engine")]
mod lifecycle;
#[cfg(feature = "engine")]
mod operations;
#[expect(
    dead_code,
    reason = "engine-absent builds retain injectable readiness tests"
)]
mod receipts;
mod schema;
mod settings;
pub use build::{ProjectionBuild, PublishedProjection};
#[cfg(feature = "engine")]
pub use handle::ProjectionHandle;
pub use import::ProjectionSnapshot;
#[cfg(feature = "engine")]
pub use lifecycle::{ProjectionFactory, ProjectionProducer};
pub use settings::{EngineSettings, ProjectionEngine};
pub use writer::{BuildVerification, CatalogRelationVocabulary};
pub(crate) mod writer;
pub use port::{
    EdgeFamily, EntityFact, InputMismatchKind, ProjectionEdge, ProjectionError, ProjectionScope,
    TypedEdgeProjection,
};
#[cfg(test)]
mod tests;
