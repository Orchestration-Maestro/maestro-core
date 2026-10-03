//! Replaceable delivery ports for client session preferences.
mod copilot;
mod preferences;
mod shared_json;
#[cfg(test)]
mod tests;
pub use copilot::{Copilot, SourceSnapshot, adapter};

pub use preferences::ClientPreferencesDelivery;
