//! Route-order results and the common first-hit deduplication rule.

use super::error::RouteError;
use crate::index::QdrantError;
use qdrant_client::qdrant::{ScoredPoint, value::Kind};
use std::collections::HashSet;

/// A chunk returned by one independent route, in its Qdrant ranking order.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredChunk {
    /// The chunk's stable ID.
    pub chunk_id: String,
    /// The source revision that owns the chunk.
    pub revision_id: String,
    /// Qdrant's score for this route.
    pub score: f64,
}

/// Keeps the first ranked hit for each chunk, stopping after `limit` chunks.
pub(crate) fn deduplicate(hits: Vec<ScoredChunk>, limit: usize) -> Vec<ScoredChunk> {
    let mut seen = HashSet::new();
    hits.into_iter()
        .filter(|hit| seen.insert(hit.chunk_id.clone()))
        .take(limit)
        .collect()
}

/// Converts Qdrant hits to their required payload fields, preserving order.
pub(super) fn chunks(
    points: Vec<ScoredPoint>,
    limit: usize,
) -> Result<Vec<ScoredChunk>, RouteError> {
    let mut hits = Vec::with_capacity(points.len());
    for point in points {
        let chunk_id = payload_text(&point, "chunk_id")?;
        let revision_id = payload_text(&point, "revision_id")?;
        if !point.score.is_finite() {
            return Err(invalid_answer("Qdrant returned a non-finite score"));
        }
        hits.push(ScoredChunk {
            chunk_id,
            revision_id,
            score: f64::from(point.score),
        });
    }
    Ok(deduplicate(hits, limit))
}

/// Gets a string payload field from one Qdrant hit.
fn payload_text(point: &ScoredPoint, key: &str) -> Result<String, RouteError> {
    match point.payload.get(key).and_then(|value| value.kind.as_ref()) {
        Some(Kind::StringValue(value)) => Ok(value.clone()),
        _ => Err(invalid_answer(&format!("hit payload lacks string {key}"))),
    }
}

/// Wraps a malformed answer from Qdrant as an invalid answer error.
fn invalid_answer(reason: &str) -> RouteError {
    RouteError::Qdrant(QdrantError::InvalidAnswer(reason.to_owned()))
}
