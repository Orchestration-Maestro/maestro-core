//! Qdrant's in-route filter for the scopes admitted to one request.

use crate::index::ProjectionFilter;
use maestro_kernel::scope::ScopeSet;

/// Matches a point when one of its `scope_tags` is explicitly granted.
pub(super) fn scope_filter(scopes: &ScopeSet) -> ProjectionFilter {
    ProjectionFilter::AnyString {
        field: "scope_tags".to_owned(),
        values: scopes
            .granted()
            .map(|scope| scope.as_str().to_owned())
            .collect(),
    }
}

/// Adds an exact metadata version to the in-Qdrant authorization filter.
pub(super) fn query_filter(scopes: &ScopeSet, version: Option<&str>) -> ProjectionFilter {
    let scope = scope_filter(scopes);
    version.map_or(scope.clone(), |version| {
        ProjectionFilter::All(vec![
            scope,
            ProjectionFilter::ExactString {
                field: "version".to_owned(),
                value: version.to_owned(),
            },
        ])
    })
}
