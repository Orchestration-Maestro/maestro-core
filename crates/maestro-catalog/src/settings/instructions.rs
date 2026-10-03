//! Conversation instructions consume the already resolved immutable preference snapshot.
use super::types::ResolvedSettings;
use crate::instructions::conversation_fragment;

/// Build the conversation fragment without reading or resolving preferences again.
/// Only canonical tags and registered tones can enter model instructions.
///
/// # Errors
/// Returns the setting key for missing, mistyped or noncanonical snapshot values.
pub fn conversation_instructions(snapshot: &ResolvedSettings) -> Result<String, String> {
    let language = snapshot.text("language").ok_or("language")?;
    let tone = snapshot.text("tone");
    conversation_fragment(language, tone)
}
