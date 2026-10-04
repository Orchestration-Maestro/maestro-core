//! MCP delivery of the session's immutable initialization instructions.
use maestro_catalog::hosts::ClientPreferencesDelivery;

/// Standard MCP initialization carries instructions identically for all clients.
pub(super) struct McpPreferencesDelivery;

impl ClientPreferencesDelivery for McpPreferencesDelivery {
    fn initialization_instructions(&self, session_preferences: &str) -> String {
        format!(
            "Search visible published collections, read their exact source-backed chunks \
             and sections, or answer from passages granted to the local principal. \
             {session_preferences}"
        )
    }
}
