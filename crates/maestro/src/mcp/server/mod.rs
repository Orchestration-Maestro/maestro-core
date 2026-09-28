//! Read-only MCP tools backed by fresh, local-principal kernel operations.

mod handler;
mod knowledge_server;
mod operations;
mod response;
#[cfg_attr(
    test,
    expect(
        clippy::self_named_module_files,
        reason = "keep the search worker tests beside the search path"
    )
)]
mod search;
#[cfg(test)]
#[expect(
    clippy::self_named_module_files,
    reason = "keep worker test modules beside the server protocol tests"
)]
mod tests;
mod types;
#[cfg_attr(
    test,
    expect(
        clippy::self_named_module_files,
        reason = "keep warm-up tests beside the startup task"
    )
)]
mod warmup;
#[cfg(test)]
mod wire_tests;

pub(super) use knowledge_server::KnowledgeServer;
