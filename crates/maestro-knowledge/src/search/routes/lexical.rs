//! BM25 diagnostic search, using the generation's recorded analyzer profile.

use super::{
    error::RouteError,
    results::{ScoredChunk, chunks, rank},
};
use crate::{
    lexical,
    search::{filter::scope_filter, query::Query},
};
use qdrant_client::qdrant::SparseVector;

/// Searches the generation's sparse `bm25` vector after analysing `text`.
///
/// # Errors
///
/// [`RouteError::ProfileMismatch`] when the generation recorded another
/// analyzer, and [`RouteError::Qdrant`] when Qdrant fails.
pub async fn search_bm25(query: &Query<'_>) -> Result<Vec<ScoredChunk>, RouteError> {
    if query.generation.sparse_profile != lexical::PROFILE {
        return Err(RouteError::ProfileMismatch {
            expected: lexical::PROFILE.to_owned(),
            found: query.generation.sparse_profile.clone(),
        });
    }
    if query.limit == 0 || query.scopes.is_empty() {
        return Ok(Vec::new());
    }
    let vector = lexical::query_vector(query.text);
    if vector.indices().is_empty() {
        return Ok(Vec::new());
    }

    let points = query
        .qdrant
        .query_sparse(
            &query.collection(),
            SparseVector {
                indices: vector.indices().to_vec(),
                values: vector.values().to_vec(),
            },
            query.limit.saturating_mul(2),
            scope_filter(query.scopes),
        )
        .await
        .map_err(RouteError::Qdrant)?;
    Ok(rank(chunks(points)?, query.limit))
}
