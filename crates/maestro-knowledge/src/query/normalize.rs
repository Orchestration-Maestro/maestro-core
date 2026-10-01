//! Whitespace normalization for queries.

use maestro_kernel::retrieval::normalize_whitespace;

/// Trims a query and collapses each whitespace run to one ASCII space,
/// preserving the case and order of every non-whitespace character.
pub(super) fn normalize(text: &str) -> String {
    normalize_whitespace(text)
}
