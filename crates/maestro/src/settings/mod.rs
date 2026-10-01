//! The session's settings, resolved once for the CLI and the MCP server, and
//! the knowledge operations' view of them.

mod knowledge;
mod session;
#[cfg(test)]
mod tests;

pub(crate) use knowledge::{Compute, KnowledgeSettings};
pub(crate) use session::Session;
