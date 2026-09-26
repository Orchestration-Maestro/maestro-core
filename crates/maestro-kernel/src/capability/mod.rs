//! Capabilities: the registry of the tools Maestro offers (building block B9).
//!
//! Every tool is registered once, with the JSON Schema of its input, its
//! effects and the scopes a caller needs, so that the CLI, the MCP server and
//! later the API show the same contract. The effects are a closed set: a new
//! one is a change to [`Effect`] that a review sees. The scopes are scope
//! paths (D4), kept as text until scopes arrive (T011). A name registered
//! twice is refused, and the registry lists its tools in name order.

mod registry;
#[cfg(test)]
mod tests;

pub use registry::{DuplicateTool, Effect, Registry, Tool};
