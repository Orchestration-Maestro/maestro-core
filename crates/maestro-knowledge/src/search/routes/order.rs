//! Stable score ties for routes whose backend traversal is unordered.

use super::results::ScoredChunk;

/// Sorts by descending score, then ascending stable chunk ID.
pub(crate) fn by_score_then_chunk_id(hits: &mut [ScoredChunk]) {
    hits.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
}
