//! Public typed-edge and literal-fact projection ports and unpublished build writer.

pub mod port;

mod content;
mod schema;
pub(crate) mod writer;
pub use port::{
    EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope, TypedEdgeProjection,
};
#[cfg(test)]
mod tests;
