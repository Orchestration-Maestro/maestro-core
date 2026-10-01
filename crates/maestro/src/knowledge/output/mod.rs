//! Shared response bounds and semantic search truncation for CLI and MCP.

mod policy;
pub(crate) use policy::*;

#[cfg(test)]
mod tests;
