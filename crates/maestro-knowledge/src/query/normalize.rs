//! Whitespace normalization for queries.

/// Trims a query and collapses each whitespace run to one ASCII space,
/// preserving the case and order of every non-whitespace character.
pub(super) fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
