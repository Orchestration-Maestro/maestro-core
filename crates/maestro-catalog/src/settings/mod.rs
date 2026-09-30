//! Typed restrictive resolution over the canonical S1 settings descriptors.

mod preferences;
mod resolve;

pub use preferences::{FilePreferences, PreferencesDraft, WorkspacePreferences, draft_preferences};

pub use resolve::{Layer, ResolveDiagnostic, ResolvedSettings, ResolvedValue, resolve};

#[cfg(test)]
mod resolve_round2;
#[cfg(test)]
mod tests;
