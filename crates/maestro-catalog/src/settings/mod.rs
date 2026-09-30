//! Typed restrictive resolution over the canonical S1 settings descriptors.

mod discovery;
mod preferences;
mod resolve;

pub use discovery::{NoWorkspaceTrust, SessionPreferences, WorkspaceTrust};

pub use preferences::{FilePreferences, PreferencesDraft, WorkspacePreferences, draft_preferences};

pub use resolve::{Layer, ResolveDiagnostic, ResolvedSettings, ResolvedValue, resolve};

#[cfg(test)]
mod resolve_round2;
#[cfg(test)]
mod tests;
