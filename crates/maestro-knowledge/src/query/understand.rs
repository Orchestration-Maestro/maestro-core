//! Public query-understanding result and entry point.

use super::identifier_types::{Family, Identifier};
use super::identifiers::find;
use super::kind::{QueryKind, classify};
use super::language::{Language, detect_language};
use super::normalize::normalize;

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
    let kind = classify(&normalized, &identifiers);
    Understood {
        normalized,
        language,
        identifiers,
        version,
        kind,
    }
}
