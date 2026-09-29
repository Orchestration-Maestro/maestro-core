//! Route-order results and the common first-hit deduplication rule.

use super::error::RouteError;
use crate::index::{PointHit, ProjectionError};
use std::collections::HashSet;

/// A chunk returned by one independent route, with its route score.
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

/// Converts Qdrant hits to their required payload fields; `rank` orders them.
pub(super) fn chunks(points: Vec<PointHit>) -> Result<Vec<ScoredChunk>, RouteError> {
    let mut hits = Vec::with_capacity(points.len());
    for point in points {
        let chunk_id = payload_text(&point, "chunk_id")?;
        let revision_id = payload_text(&point, "revision_id")?;
        let score = point
            .score
            .ok_or_else(|| invalid_answer("ranked hit lacks a score"))?;
        if !score.is_finite() {
            return Err(invalid_answer("Qdrant returned a non-finite score"));
        }
        hits.push(ScoredChunk {
            chunk_id,
            revision_id,
            score,
        });
    }
    Ok(hits)
}

/// Orders near-equal scores by ID, then removes duplicates and applies `limit`.
///
/// Qdrant's IDF-weighted f32 scores vary by a few ULPs between builds, far
/// below τ = 1e-5 × max(1, |group leader|); meaningful score gaps are far larger.
/// A tie group larger than the extra `limit` fetched by each route can still
/// be truncated by Qdrant, leaving its cutoff order-dependent.
pub(crate) fn rank(hits: Vec<ScoredChunk>, limit: usize) -> Vec<ScoredChunk> {
    let mut hits = hits;
    hits.sort_by(|left, right| right.score.total_cmp(&left.score));
    let hits_len = hits.len();
    let mut group_start = 0;
    while let Some(leader) = hits.get(group_start).map(|hit| hit.score) {
        let Some(group_tail) = hits.get_mut(group_start..) else {
            break;
        };
        let tolerance = 1e-5 * leader.abs().max(1.0);
        let group_len = group_tail
            .iter()
            .take_while(|hit| leader - hit.score <= tolerance)
            .count()
            .max(1); // Always advance past the leader.
        let (group, remaining) = group_tail.split_at_mut(group_len);
        group.sort_by(|left, right| left.chunk_id.cmp(&right.chunk_id));
        group_start = hits_len.saturating_sub(remaining.len());
    }
    deduplicate(hits, limit)
}

/// Gets a string payload field from one Qdrant hit.
fn payload_text(point: &PointHit, key: &str) -> Result<String, RouteError> {
    point
        .payload
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid_answer(&format!("hit payload lacks string {key}")))
}

/// Wraps a malformed answer from Qdrant as an invalid answer error.
fn invalid_answer(reason: &str) -> RouteError {
    RouteError::Qdrant(ProjectionError::new(format!(
        "Qdrant's answer is not what was asked for: {reason}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_within_the_tie_tolerance_are_ordered_by_id_independent_of_input_order() {
        let first = rank(
            vec![hit("c", 1.0), hit("a", 0.999_995), hit("b", 0.999_991)],
            3,
        );
        let second = rank(
            vec![hit("b", 0.999_991), hit("c", 1.0), hit("a", 0.999_995)],
            3,
        );
        assert_eq!(first, second);
        assert_eq!(ids(&first), ["a", "b", "c"]);
    }

    #[test]
    fn score_gaps_larger_than_the_tie_tolerance_keep_score_order() {
        let result = rank(vec![hit("a", 0.999_98), hit("z", 1.0)], 2);
        assert_eq!(ids(&result), ["z", "a"]);
    }

    #[test]
    fn scaled_tolerance_groups_ulp_noise_but_keeps_real_score_gaps() {
        let leader = 10.0_f32;
        let one_ulp_below = f32::from_bits(leader.to_bits() - 1);
        let two_ulps_below = f32::from_bits(leader.to_bits() - 2);
        let result = rank(
            vec![
                hit("z", leader),
                hit("b", one_ulp_below),
                hit("a", two_ulps_below),
                hit("m", 9.99),
            ],
            4,
        );
        assert_eq!(ids(&result), ["a", "b", "z", "m"]);
    }

    #[test]
    fn tie_group_crossing_the_limit_keeps_the_lowest_chunk_ids() {
        let result = rank(
            vec![
                hit("z", 1.0),
                hit("b", 0.999_996),
                hit("a", 0.999_993),
                hit("outside-group", 0.5),
            ],
            2,
        );
        assert_eq!(ids(&result), ["a", "b"]);
    }

    fn hit(chunk_id: &str, score: f32) -> ScoredChunk {
        ScoredChunk {
            chunk_id: chunk_id.to_owned(),
            revision_id: "revision".to_owned(),
            score: f64::from(score),
        }
    }

    fn ids(chunks: &[ScoredChunk]) -> Vec<&str> {
        chunks.iter().map(|chunk| chunk.chunk_id.as_str()).collect()
    }
}
