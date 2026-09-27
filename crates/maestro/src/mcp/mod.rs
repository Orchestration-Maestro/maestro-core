//! Bounded local stdio MCP transport and tools.

pub(crate) mod run;
#[cfg_attr(
    test,
    expect(
        clippy::self_named_module_files,
        reason = "keep the handler in server.rs and its unit tests in server/"
    )
)]
mod server;
#[cfg_attr(
    test,
    expect(
        clippy::self_named_module_files,
        reason = "keep stdio framing in transport.rs and its unit tests in transport/"
    )
)]
mod transport;
