//! The single fixed English rule and conversation/native fragment constructors.
use maestro_settings::{AUTO, canonical_language};

/// Shared invariant for model delivery and value-free native projections.
pub const ARTIFACT_LOG_RULE: &str =
    "Keep code, commits, names, identifiers, logs and documentation in English.";

/// Construct the exact C05e fragment from canonical values in an immutable snapshot.
pub(crate) fn conversation_fragment(language: &str, tone: Option<&str>) -> Result<String, String> {
    if language != AUTO && canonical_language(language).as_deref() != Ok(language) {
        return Err("language".to_owned());
    }
    let tone = tone.ok_or("tone")?;
    if !["brief", "normal", "detailed"].contains(&tone) {
        return Err("tone".to_owned());
    }
    Ok(format!(
        "Conversation language: \"{language}\"; tone: \"{tone}\". \
         When language is \"auto\", follow the question's language. {ARTIFACT_LOG_RULE}"
    ))
}

/// Native delivery never persists language or tone values; the current MCP session supplies them.
pub(crate) fn native_preferences_instructions() -> String {
    format!("Follow the current MCP session's conversation language and tone. {ARTIFACT_LOG_RULE}")
}
