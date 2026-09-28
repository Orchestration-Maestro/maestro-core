//! Scoped knowledge operations shared by the CLI and stdio MCP server.

/// Application operations shared by the CLI and MCP boundaries.
pub(crate) mod operations;
pub(crate) mod output;
mod requests;

#[cfg(test)]
pub(crate) use operations::tests::Scratch as RefreshScratch;
pub(crate) use output::RESPONSE_LIMIT_BYTES;
pub(crate) use requests::{GetRequest, RequestError, SearchRequest};
