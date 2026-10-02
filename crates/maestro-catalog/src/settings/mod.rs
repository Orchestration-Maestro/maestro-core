//! Typed restrictive resolution over the canonical S1 settings descriptors.

pub(crate) mod defaults;
mod discovery;
mod preferences;
pub(crate) mod resolve;
mod session;
mod standards;
mod types;

pub use discovery::{NoWorkspaceTrust, SessionPreferences, WorkspaceTrust};

pub use preferences::{FilePreferences, PreferencesDraft, WorkspacePreferences, draft_preferences};

pub use resolve::resolve;
pub use types::{Layer, ResolveDiagnostic, ResolvedSettings, ResolvedValue};

#[cfg(test)]
mod resolve_round2;
#[cfg(test)]
mod tests;
