//! Read-only MCP tools backed by fresh, local-principal kernel operations.

mod handler;
mod knowledge_server;
mod operations;
mod response;
mod search;
#[cfg(test)]
mod tests;
mod types;
mod warmup;
#[cfg(test)]
mod wire_tests;

pub(super) use knowledge_server::KnowledgeServer;
