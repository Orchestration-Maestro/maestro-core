//! Public query-understanding result and entry point.

use super::identifier_types::{Family, Identifier};
use super::identifiers::find;
use super::kind::{QueryKind, classify};
use super::language::{Language, detect_language};
use super::normalize::normalize;

/// Exact suffixes introducing the inventory grammar's JSON set string.
pub(crate) const INVENTORY_FILTERS: &[&str] = &[" in set", " dans le lot"];

/// Deterministic, low-cost understanding of a search query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Understood {
    /// Trimmed text with whitespace runs collapsed and case preserved.
    pub normalized: String,
    /// Detected language, or [`Language::Unknown`] when uncertain.
    pub language: Language,
    /// Exact identifier spans in source order.
    pub identifiers: Vec<Identifier>,
    /// The first detected version identifier, if present.
    pub version: Option<String>,
    /// The first matching query-kind rule.
    pub kind: QueryKind,
}

/// Masks the JSON inventory value so its words cannot trigger query-kind rules.
fn classification_text(text: &str) -> String {
    let Some(quote) = text.char_indices().find_map(|(index, character)| {
        if character != '"' {
            return None;
        }
        let prefix = text.get(..index)?.trim_end();
        INVENTORY_FILTERS
            .iter()
            .any(|suffix| {
                prefix
                    .get(prefix.len().saturating_sub(suffix.len())..)
                    .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
            })
            .then_some(index)
    }) else {
        return text.to_owned();
    };
    text.get(..quote).unwrap_or_default().to_owned()
}

/// Normalizes and classifies a query before retrieval routes consume it.
#[must_use]
pub fn understand(text: &str) -> Understood {
    let normalized = normalize(text);
    let language = detect_language(&normalized);
    let identifiers = find(&normalized);
    let version = identifiers
        .iter()
        .find(|identifier| identifier.family == Family::Version)
        .map(|identifier| identifier.text.clone());
    let classification = classification_text(&normalized);
    let kind = classify(&classification, &find(&classification));
    Understood {
        normalized,
        language,
        identifiers,
        version,
        kind,
    }
}
