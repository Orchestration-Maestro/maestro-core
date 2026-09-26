//! The structural check of a generation's collection, before its alias
//! moves: its vectors are those its profiles call for, and it holds one
//! point for each chunk of its set, and no other. The retrieval smoke checks
//! join it with the publish command (T028).

use super::{
    error::Unverified,
    point::point_id,
    qdrant::{DENSE, Qdrant, QdrantError, SPARSE},
};
use maestro_kernel::chunk_set::Chunk;
use qdrant_client::qdrant::{CollectionParams, Distance, Modifier, vectors_config::Config};
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
/// [`QdrantError`] when Qdrant fails, which checks nothing.
pub(super) async fn verify(
    qdrant: &Qdrant,
    collection: &str,
    dimensions: u64,
    chunks: &[Chunk],
) -> Result<Result<u64, Unverified>, QdrantError> {
    if let Err(unverified) = vectors(&qdrant.parameters(collection).await?, dimensions) {
        return Ok(Err(unverified));
    }
    let expected = u64::try_from(chunks.len()).unwrap_or(u64::MAX);
    let found = qdrant.count(collection).await?;
    if found != expected {
        return Ok(Err(Unverified::Count { expected, found }));
    }
    for batch in chunks.chunks(LOOKUP) {
        let ids: Vec<String> = batch.iter().map(|chunk| point_id(&chunk.id)).collect();
        let present: HashSet<String> = qdrant.found(collection, &ids).await?.into_iter().collect();
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
pub(super) fn vectors(parameters: &CollectionParams, dimensions: u64) -> Result<(), Unverified> {
    let dense = match parameters
        .vectors_config
        .as_ref()
        .and_then(|config| config.config.as_ref())
    {
        Some(Config::ParamsMap(named)) => named.map.get(DENSE),
        Some(Config::Params(_)) | None => None,
    };
    let modifier = parameters
        .sparse_vectors_config
        .as_ref()
        .and_then(|sparse| sparse.map.get(SPARSE))
        .map(|sparse| sparse.modifier);
    let matches = dense.is_some_and(|dense| {
        dense.size == dimensions && dense.distance == i32::from(Distance::Cosine)
    }) && modifier == Some(Some(i32::from(Modifier::Idf)));
    if matches {
        return Ok(());
    }
    let dense = dense.map_or_else(
        || format!("no dense vector `{DENSE}`"),
        |dense| {
            let distance = Distance::try_from(dense.distance)
                .map_or("unknown", |distance| distance.as_str_name());
            format!(
                "a dense vector `{DENSE}` of {} dimensions compared by {distance}",
                dense.size
            )
        },
    );
    let sparse = modifier.map_or_else(
        || format!("no sparse vector `{SPARSE}`"),
        |modifier| {
            let modifier = modifier
                .and_then(|modifier| Modifier::try_from(modifier).ok())
                .map_or("no", |modifier| modifier.as_str_name());
            format!("a sparse vector `{SPARSE}` weighted by {modifier} modifier")
        },
    );
    Err(Unverified::Vectors {
        dimensions,
        found: format!("{dense} and {sparse}"),
    })
}
