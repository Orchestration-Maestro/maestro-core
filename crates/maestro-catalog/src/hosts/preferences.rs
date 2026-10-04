//! Delivery boundary independent of client names and preference storage.
use crate::instructions::native_preferences_instructions;

/// Project the immutable, validated session instructions into a client payload.
/// A new client supplies an adapter; callers never branch on its identity.
pub trait ClientPreferencesDelivery {
    /// Return the initialization payload for the already constructed snapshot.
    /// The adapter must not reread files or accept model-supplied replacements.
    fn initialization_instructions(&self, session_preferences: &str) -> String;

    /// Deliver the shared value-free English rule through this client's native adapter.
    /// The current MCP session, never a projected file, supplies conversation preferences.
    fn native_instructions(&self) -> String {
        self.initialization_instructions(&native_preferences_instructions())
    }
}
