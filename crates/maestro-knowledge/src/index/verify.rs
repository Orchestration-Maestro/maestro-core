//! The structural check of a generation's collection, before its alias
//! moves: its vectors are those its profiles call for, and it holds one
//! point for each chunk of its set, and no other. The retrieval smoke checks
//! join it with the publish command (T028).

use super::{
    error::Unverified,
    point::point_id,
    projection_port::{CollectionLayout, ProjectionError, RetrievalProjectionPort},
};
use maestro_kernel::chunk_set::Chunk;
use std::collections::HashSet;

/// How many point IDs one request looks up.
const LOOKUP: usize = 1000;

/// Checks the collection `collection` of a generation whose embedder gives
/// `dimensions` and whose chunk set holds `chunks`, and returns the points
/// it counted, or the first check it failed: its vectors, its count, then
/// the point of every chunk. The count and the points together mean it
/// holds the chunks' points and no other.
///
/// # Errors
///
/// [`ProjectionError`] when the projection backend fails, which checks nothing.
pub(super) async fn verify(
    qdrant: &impl RetrievalProjectionPort<Error = ProjectionError>,
    collection: &str,
    dimensions: u64,
    chunks: &[Chunk],
) -> Result<Result<u64, Unverified>, ProjectionError> {
    let Some(layout) = qdrant.collection_layout(collection).await? else {
        return Ok(Err(Unverified::NoCollection));
    };
    if let Err(unverified) = vectors(&layout, dimensions) {
        return Ok(Err(unverified));
    }
    let expected = u64::try_from(chunks.len()).unwrap_or(u64::MAX);
    let found = qdrant.count_points(collection).await?;
    if found != expected {
        return Ok(Err(Unverified::Count { expected, found }));
    }
    for batch in chunks.chunks(LOOKUP) {
        let ids: Vec<String> = batch.iter().map(|chunk| point_id(&chunk.id)).collect();
        let present: HashSet<String> = qdrant
            .point_ids(collection, &ids)
            .await?
            .into_iter()
            .collect();
        let missing = batch
            .iter()
            .zip(&ids)
            .find(|(_, id)| !present.contains(*id));
        if let Some((chunk, _)) = missing {
            return Ok(Err(Unverified::Missing {
                chunk: chunk.id.clone(),
            }));
        }
    }
    Ok(Ok(found))
}

/// Refuses a collection of `parameters` unless its dense vector [`DENSE`]
/// has `dimensions` compared by cosine and its sparse vector [`SPARSE`] is
/// weighted by IDF.
pub(super) fn vectors(layout: &CollectionLayout, dimensions: u64) -> Result<(), Unverified> {
    let matches = layout.dense_present
        && layout.dense_dimensions == dimensions
        && layout.dense_distance == "Cosine"
        && layout.sparse_present
        && layout.sparse_modifier.as_deref() == Some("Idf");
    if matches {
        return Ok(());
    }
    let dense = if layout.dense_present {
        format!(
            "a dense vector `dense` of {} dimensions compared by {}",
            layout.dense_dimensions,
            layout.dense_distance.to_ascii_uppercase()
        )
    } else {
        "no dense vector `dense`".to_owned()
    };
    let sparse = if layout.sparse_present {
        format!(
            "a sparse vector `bm25` weighted by {} modifier",
            layout
                .sparse_modifier
                .as_deref()
                .unwrap_or("no")
                .to_ascii_uppercase()
        )
    } else {
        "no sparse vector `bm25`".to_owned()
    };
    Err(Unverified::Vectors {
        dimensions,
        found: format!("{dense} and {sparse}"),
    })
}
