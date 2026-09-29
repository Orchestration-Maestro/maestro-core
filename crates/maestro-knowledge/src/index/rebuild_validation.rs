//! Validation of the frozen identity for a guarded projection rebuild.

use super::error::Error;
use crate::lexical;
use maestro_kernel::{chunk_set::ChunkSet, generation::Generation};

/// Confirms a candidate belongs to this set, its restored card and profile.
pub(super) fn validate_tuple(
    generation: &Generation,
    set: &ChunkSet,
    embedding: &str,
    watermark: i64,
) -> Result<(), Error> {
    if generation.id <= watermark
        || generation.collection_id != set.collection_id
        || generation.chunk_set_id != set.id
        || generation.embedding_profile != embedding
        || generation.sparse_profile != lexical::PROFILE
    {
        return Err(Error::RecoveryTarget {
            generation: generation.id,
        });
    }
    Ok(())
}
