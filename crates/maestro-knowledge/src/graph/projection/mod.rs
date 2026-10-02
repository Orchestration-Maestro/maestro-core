//! Public typed-edge and literal-fact projection ports and unpublished build writer.

pub mod port;

mod content;
#[cfg(feature = "engine")]
mod engine;
#[expect(
    dead_code,
    reason = "the E09 health adapter consumes this readiness port"
)]
mod receipts;
mod schema;
pub(crate) mod writer;
pub use port::{
    EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope, TypedEdgeProjection,
};
#[cfg(test)]
mod tests;
