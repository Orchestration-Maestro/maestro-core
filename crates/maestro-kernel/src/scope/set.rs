//! The scopes a principal may read, found for one request, and the condition
//! a reader's query filters by.

use super::path::{Scope, WORKSPACE};
use serde_json::Value;
use std::collections::BTreeSet;

/// The scopes a principal may read, as [`Database::visible`] found them for
/// one request: each scope granted to it, and every scope below one. Keep it
/// for that request and never longer, so a revocation applies to the next
/// read. Outside the kernel only `visible` gives one, and an empty set sees
/// nothing.
///
/// [`Database::visible`]: crate::store::Database::visible
#[derive(Debug, PartialEq, Eq)]
pub struct ScopeSet(BTreeSet<Scope>);

impl ScopeSet {
    /// The set of the `granted` scopes and every scope below them. It is the
    /// crate's own: outside the kernel, only the grants of a principal give
    /// a set.
    pub(crate) fn new(granted: BTreeSet<Scope>) -> Self {
        Self(granted)
    }

    /// Whether `scope` is in the set: granted, or below a granted scope.
    #[must_use]
    pub fn covers(&self, scope: &Scope) -> bool {
        self.0.iter().any(|granted| granted.covers(scope))
    }

    /// Whether the set is empty, and so sees nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The scopes the set was granted, in path order; each covers every
    /// scope below it too.
    pub fn granted(&self) -> impl Iterator<Item = &Scope> {
        self.0.iter()
    }

    /// The granted scopes' paths, as the JSON array a query binds to the
    /// parameter of [`ScopeSet::condition`].
    pub(crate) fn parameter(&self) -> String {
        Value::from_iter(self.0.iter().map(Scope::as_str)).to_string()
    }

    /// The SQL condition that holds when the scope path `path`, a column or
    /// an expression of text, is in the set bound to the query's parameter
    /// `parameter` ([`ScopeSet::parameter`]): a granted path, or one below
    /// it, compared by whole segments rather than by a pattern. The
    /// condition's subquery has columns of its own (`json_each`'s `id`,
    /// `key`, `value`, `path`…), so every column the expression names
    /// carries its table's name.
    pub(crate) fn condition(path: &str, parameter: usize) -> String {
        format!(
            "EXISTS (SELECT 1 FROM json_each(?{parameter}) AS granted
                     WHERE {path} = granted.value
                        OR substr({path}, 1, length(granted.value) + 1) = granted.value || '/')"
        )
    }

    /// [`ScopeSet::condition`] for the collection whose id is the column
    /// `collection`, whose scope is `workspace/default/collection/<id>`, as
    /// [`collection_path`](super::collection_path) writes it.
    pub(crate) fn collection_condition(collection: &str, parameter: usize) -> String {
        Self::condition(
            &format!("('{WORKSPACE}/collection/' || {collection})"),
            parameter,
        )
    }

    /// [`ScopeSet::condition`] for the source whose id is the column `source`
    /// in the collection whose id is the column `collection`, whose scope is
    /// `workspace/default/collection/<id>/source/<id>`, as
    /// [`source_path`](super::source_path) writes it.
    pub(crate) fn source_condition(collection: &str, source: &str, parameter: usize) -> String {
        Self::condition(
            &format!("('{WORKSPACE}/collection/' || {collection} || '/source/' || {source})"),
            parameter,
        )
    }
}

#[cfg(test)]
impl ScopeSet {
    /// The set that covers the whole default workspace, where the kernel
    /// keeps its records: what the kernel's tests read with when scopes are
    /// not what they test.
    pub(crate) fn default_workspace() -> Self {
        Self::new(BTreeSet::from([WORKSPACE.parse().unwrap()]))
    }
}
