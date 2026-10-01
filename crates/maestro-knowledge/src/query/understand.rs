//! Public query-understanding result and entry point.

use super::identifier_types::{Family, Identifier};
use super::identifiers::find;
use super::kind::{QueryKind, classify};
use super::language::{Language, detect_language};
use super::normalize::normalize;
use crate::lexical;

/// Exact English and French phrases that request document counts or sets.
pub(crate) const INVENTORY_DOCUMENT_FORMS: &[&str] = &[
    "how many documents",
    "combien de documents",
    "list all document sets",
    "liste des lots de documents",
];
/// Exact English and French phrases that request documented versions.
pub(crate) const INVENTORY_VERSION_FORMS: &[&str] = &["list all versions", "liste des versions"];
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
        let prefix = fold_inventory_keywords(text.get(..index)?.trim_end());
        INVENTORY_DOCUMENT_FORMS
            .iter()
            .chain(INVENTORY_VERSION_FORMS)
            .any(|form| {
                let form = fold_inventory_keywords(form);
                INVENTORY_FILTERS
                    .iter()
                    .any(|suffix| prefix == format!("{form}{suffix}"))
            })
            .then_some(index)
    }) else {
        return text.to_owned();
    };
    text.get(..quote).unwrap_or_default().to_owned()
}

/// Folds only inventory keywords, preserving the exact filter string.
fn fold_inventory_keywords(text: &str) -> String {
    lexical::fold(text).to_lowercase()
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
