//! User-approved workspace records with a non-replaceable root refusal floor.
mod approval;
mod deny;
mod paths;
mod port;
mod replacement;
#[cfg(test)]
mod tests;

pub use approval::{
    PreferencesConfirmation, confirmation, preferences_confirmation, write_preferences,
};
pub use paths::{Access, AuthorizedPath};
pub use port::{CheckedTrust, JournalTrust, TrustBoundaries, WorkspaceTrust};
