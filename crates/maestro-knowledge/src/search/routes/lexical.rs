//! BM25 diagnostic search, using the generation's recorded analyzer profile.

use super::{
    error::RouteError,
    results::{ScoredChunk, chunks, rank},
};
use crate::{
    index::{RetrievalProjectionPort, SparseValues},
    lexical,
    search::{filter::query_filter, query::Query},
};

/// Searches the generation's sparse `bm25` vector after analysing `text`.
///
/// # Errors
///
/// [`RouteError::ProfileMismatch`] when the generation recorded another
/// analyzer, and [`RouteError::Projection`] when projection backend fails.
pub async fn search_bm25<R: RetrievalProjectionPort>(
    query: &Query<'_, R>,
) -> Result<Vec<ScoredChunk>, RouteError> {
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
        .projection
        .search_sparse(
            &query.collection(),
            SparseValues {
                indices: vector.indices().to_vec(),
                values: vector.values().to_vec(),
            },
            query.limit.saturating_mul(2),
            query_filter(query.scopes, query.version),
        )
        .await
        .map_err(RouteError::Projection)?;
    Ok(rank(chunks(points)?, query.limit))
}
