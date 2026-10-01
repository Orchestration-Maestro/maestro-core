//! Exact identifiers to store beside published prepared inputs.

use super::{identifiers::candidates, normalize::normalize};
use std::collections::BTreeSet;

/// Extracts every pattern-family identifier before query overlap arbitration.
pub(crate) fn index_identifiers(text: &str) -> Vec<String> {
    let normalized = normalize(text);
    candidates(&normalized)
        .into_iter()
        .filter_map(|candidate| normalized.get(candidate.start..candidate.end))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
