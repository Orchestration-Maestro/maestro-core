//! User-approved workspace records with a non-replaceable root refusal floor.
mod approval;
mod port;
#[cfg(test)]
mod tests;

pub use approval::{confirmation, write_preferences};
pub use port::{CheckedTrust, JournalTrust, TrustBoundaries, WorkspaceTrust};
