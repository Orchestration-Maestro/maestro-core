//! The session's settings, resolved once for the CLI and the MCP server, and
//! the knowledge operations' view of them.

mod graph;
#[cfg(any(feature = "engine", test))]
mod graph_native;
#[cfg(test)]
mod graph_tests;
mod knowledge;
mod session;
#[cfg(test)]
mod tests;

pub(crate) use graph::GraphEngine;
pub(crate) use knowledge::{Compute, KnowledgeSettings};
pub(crate) use session::{GraphActivationError, Session};
