//! Dense diagnostic search through the generation's own embedder and vector.

use super::{
    error::{RouteError, VectorError},
    results::{ScoredChunk, chunks, rank},
};
use crate::{
    index::{RetrievalProjectionPort, embedding_profile},
    search::{filter::query_filter, query::Query},
};
use maestro_kernel::gateway::{ModelCard, ModelPort, Role, Room};
use std::{num::NonZeroUsize, slice, time::Duration};
use tokio::time;

/// Maximum time a route waits for its query embedding.
const EMBEDDING_DEADLINE: Duration = Duration::from_secs(60);

/// The embedder that owns the dense vectors of a generation.
#[derive(Debug)]
pub struct Embedder<'a, P> {
    /// The model port used to embed the query.
    pub port: &'a P,
    /// The card whose profile the generation recorded.
    pub card: &'a ModelCard,
}

/// Searches the generation's dense vectors after embedding in `Room::Free`.
///
/// Each document token-count and embedding request formats its raw input once.
///
/// # Errors
///
/// [`RouteError::ProfileMismatch`] when the card is not the generation's
/// embedder, [`RouteError::EmbedderUnavailable`] when its model port refuses,
/// has no free room, times out or is unreachable, [`RouteError::InvalidVector`]
/// when its response is malformed, and [`RouteError::Projection`] when the
/// projection backend fails.
pub async fn search_dense<P: ModelPort, R: RetrievalProjectionPort>(
    query: &Query<'_, R>,
    embedder: &Embedder<'_, P>,
) -> Result<Vec<ScoredChunk>, RouteError> {
    let profile = embedding_profile(embedder.card);
    if profile != query.generation.embedding_profile {
        return Err(RouteError::ProfileMismatch {
            expected: query.generation.embedding_profile.clone(),
            found: profile,
        });
    }
    let (Role::Embedder, Some(dimensions)) = (
        embedder.card.fields().role,
        embedder.card.fields().dimensions,
    ) else {
        return Err(RouteError::EmbedderUnavailable {
            reason: "the generation's model card is not an embedder".to_owned(),
        });
    };
    if query.limit == 0 || query.scopes.is_empty() {
        return Ok(Vec::new());
    }

    let input = embedder.card.format_query(query.text);
    let vectors = time::timeout(
        EMBEDDING_DEADLINE,
        embedder
            .port
            .embed(embedder.card, Room::Free, slice::from_ref(&input)),
    )
    .await
    .map_err(|_| RouteError::EmbedderUnavailable {
        reason: format!(
            "query embedding timed out after {} s",
            EMBEDDING_DEADLINE.as_secs()
        ),
    })?
    .map_err(|error| RouteError::EmbedderUnavailable {
        reason: error.to_string(),
    })?;
    let [vector] = vectors.as_slice() else {
        return Err(RouteError::InvalidVector(VectorError::Count {
            found: vectors.len(),
        }));
    };
    check_vector(vector, dimensions)?;

    let points = query
        .projection
        .search_dense(
            &query.collection(),
            vector.clone(),
            query.limit.saturating_mul(2),
            query_filter(query.scopes, query.version),
        )
        .await
        .map_err(RouteError::Projection)?;
    Ok(rank(chunks(points)?, query.limit))
}

/// Refuses an embedding whose shape or values cannot be used by the projection backend.
fn check_vector(vector: &[f32], dimensions: NonZeroUsize) -> Result<(), RouteError> {
    let expected = dimensions.get();
    if vector.len() != expected {
        return Err(RouteError::InvalidVector(VectorError::Dimensions {
            expected,
            found: vector.len(),
        }));
    }
    if !vector.iter().all(|value| value.is_finite()) {
        return Err(RouteError::InvalidVector(VectorError::NonFinite));
    }
    if vector.iter().all(|value| *value == 0.0) {
        return Err(RouteError::InvalidVector(VectorError::Zero));
    }
    Ok(())
}
