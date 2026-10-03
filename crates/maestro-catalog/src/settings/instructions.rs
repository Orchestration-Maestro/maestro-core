//! English model instructions over already resolved session preferences.
use super::types::ResolvedSettings;
use maestro_settings::{AUTO, canonical_language};

/// Shared invariant for model delivery and value-free native projections.
pub const ARTIFACT_LOG_RULE: &str =
    "Keep code, commits, names, identifiers, logs and documentation in English.";

/// Build the conversation fragment without reading or resolving preferences again.
/// Only canonical tags and registered tones can enter model instructions.
///
/// # Errors
/// Returns the setting key for missing, mistyped or noncanonical snapshot values.
pub fn conversation_instructions(snapshot: &ResolvedSettings) -> Result<String, String> {
    let language = snapshot.text("language").ok_or("language")?;
    if language != AUTO && canonical_language(language).as_deref() != Ok(language) {
        return Err("language".to_owned());
    }
    let tone = snapshot.text("tone").ok_or("tone")?;
    if !["brief", "normal", "detailed"].contains(&tone) {
        return Err("tone".to_owned());
    }
    Ok(format!(
        "Conversation language: \"{language}\"; tone: \"{tone}\". \
         When language is \"auto\", follow the question's language. {ARTIFACT_LOG_RULE}"
    ))
}

/// Input for native delivery adapters: no persisted language or tone values.
#[must_use]
pub fn native_preferences_instructions() -> String {
    format!("Follow the current MCP session's conversation language and tone. {ARTIFACT_LOG_RULE}")
}
