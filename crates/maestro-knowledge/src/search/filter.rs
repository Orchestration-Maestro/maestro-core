//! Qdrant's in-route filter for the scopes admitted to one request.

use maestro_kernel::scope::ScopeSet;
use qdrant_client::qdrant::{Condition, Filter};

/// Matches a point when one of its `scope_tags` is explicitly granted.
pub(super) fn scope_filter(scopes: &ScopeSet) -> Filter {
    Filter::must([Condition::matches(
        "scope_tags",
        scopes
            .granted()
            .map(|scope| scope.as_str().to_owned())
            .collect::<Vec<_>>(),
    )])
}

/// Adds an exact metadata version to the in-Qdrant authorization filter.
pub(super) fn query_filter(scopes: &ScopeSet, version: Option<&str>) -> Filter {
    let mut filter = scope_filter(scopes);
    if let Some(version) = version {
        filter
            .must
            .push(Condition::matches("version", version.to_owned()));
    }
    filter
}
