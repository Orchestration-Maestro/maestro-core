//! The scopes and generation admitted for a search.

use crate::index::{Qdrant, collection_name};
use maestro_kernel::{generation::Generation, scope::ScopeSet};

/// The request shared by the dense and BM25 diagnostic routes.
#[derive(Debug)]
pub struct Query<'a, R = Qdrant> {
    /// The generation pinned at admission; routes keep using it if the alias moves.
    pub generation: &'a Generation,
    /// The caller's current read scopes.
    pub scopes: &'a ScopeSet,
    /// The text to search.
    pub text: &'a str,
    /// The maximum number of chunks to return.
    pub limit: usize,
    /// The exact version filter selected for this request.
    pub version: Option<&'a str>,
    /// The backend holding the generation's collection.
    pub qdrant: &'a R,
}

impl<R> Query<'_, R> {
    /// The immutable collection of this generation, never its alias.
    pub(super) fn collection(&self) -> String {
        collection_name(self.generation)
    }
}
