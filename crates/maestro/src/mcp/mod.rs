//! Bounded local stdio MCP transport and tools.

/// `knowledge_ask` request validation and JSON Schemas.
mod ask_tool;
mod outcome;
pub(crate) mod run;
mod server;
#[cfg_attr(
    test,
    expect(
        clippy::self_named_module_files,
        reason = "keep stdio framing in transport.rs and its unit tests in transport/"
    )
)]
mod transport;
