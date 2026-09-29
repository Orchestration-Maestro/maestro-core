//! Verification of a published generation against the kernel artifacts and
//! the Qdrant alias it serves.

use crate::index::{QdrantError, RetrievalProjectionPort, alias_name, collection_name};
use maestro_kernel::{
    artifact,
    chunk_set::{self, ChunkSetState},
    generation::{self, GenerationState},
    scope::ScopeSet,
    store::{self, Database},
};
use serde::Serialize;
use std::{error, fmt};

/// What verification found about a published generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verification {
    /// The collection the generation serves.
    pub collection: String,
    /// The published generation.
    pub generation: i64,
    /// The chunk set it was built from.
    pub chunk_set: String,
    /// The count Qdrant returned.
    pub point_count: u64,
    /// Each invariant the generation failed, named for a person.
    pub findings: Vec<String>,
}

/// Why verification could not read the generation, its chunks or Qdrant.
#[derive(Debug)]
pub enum Error {
    /// The generation is unknown to the caller's scopes.
    UnknownGeneration(i64),
    /// The chunk set named by a generation is unknown to the caller's scopes.
    UnknownChunkSet(String),
    /// The kernel could not read generation state.
    Generation(generation::Error),
    /// The kernel could not read the chunk set or its chunks.
    ChunkSet(chunk_set::Error),
    /// The kernel could not read an artifact for reasons other than absence
    /// or a digest mismatch.
    Artifact(store::Error),
    /// Qdrant could not answer a verification request.
    Qdrant(QdrantError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownGeneration(id) => write!(formatter, "no generation {id} is recorded"),
            Self::UnknownChunkSet(id) => write!(formatter, "no chunk set {id} is recorded"),
            Self::Generation(error) => write!(formatter, "the generation failed: {error}"),
            Self::ChunkSet(error) => write!(formatter, "the chunk set failed: {error}"),
            Self::Artifact(error) => {
                write!(formatter, "the prepared input artifact failed: {error}")
            }
            Self::Qdrant(error) => write!(formatter, "Qdrant failed: {error}"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Generation(error) => Some(error),
            Self::ChunkSet(error) => Some(error),
            Self::Artifact(error) => Some(error),
            Self::Qdrant(error) => Some(error),
            Self::UnknownGeneration(_) | Self::UnknownChunkSet(_) => None,
        }
    }
}

/// Verifies generation `id`: every prepared input artifact matches its digest,
/// Qdrant has one point for every chunk and the expected alias names its
/// collection. Findings are returned together; an inability to read state is
/// an error instead.
///
/// # Errors
///
/// [`Error::UnknownGeneration`] and [`Error::UnknownChunkSet`] when the
/// caller's scopes read neither, [`Error::Generation`] or [`Error::ChunkSet`]
/// when the kernel fails, [`Error::Artifact`] for another artifact-store
/// failure, and [`Error::Qdrant`] when Qdrant fails.
pub async fn verify_generation(
    database: &Database,
    scopes: &ScopeSet,
    qdrant: &impl RetrievalProjectionPort<Error = QdrantError>,
    id: i64,
) -> Result<Verification, Error> {
    let generation = database
        .generation(scopes, id)
        .map_err(Error::Generation)?
        .ok_or(Error::UnknownGeneration(id))?;
    let set = database
        .chunk_set(scopes, &generation.chunk_set_id)
        .map_err(Error::ChunkSet)?
        .ok_or_else(|| Error::UnknownChunkSet(generation.chunk_set_id.clone()))?;
    let chunks = database.chunks(scopes, &set.id).map_err(Error::ChunkSet)?;
    let mut findings = Vec::new();
    if generation.state != GenerationState::Published {
        findings.push(format!(
            "generation {} is {}, not published",
            generation.id, generation.state
        ));
    }
    if set.state != ChunkSetState::Complete {
        findings.push(format!(
            "chunk set {} is {}, not complete",
            set.id, set.state
        ));
    }
    for chunk in &chunks {
        match database.get(&chunk.digest) {
            Ok(_) => {}
            Err(
                store::Error::UnknownArtifact(_)
                | store::Error::Artifact(artifact::Error::Missing(_)),
            ) => {
                findings.push(format!(
                    "missing artifact {} for prepared input of chunk {}",
                    chunk.digest.as_str(),
                    chunk.id
                ));
            }
            Err(store::Error::Artifact(artifact::Error::Corrupt { found, .. })) => {
                findings.push(format!(
                    "chunk {} prepared input digest mismatch: expected {}, found {}",
                    chunk.id,
                    chunk.digest.as_str(),
                    found.as_str()
                ));
            }
            Err(error) => return Err(Error::Artifact(error)),
        }
    }
    let qdrant_collection = collection_name(&generation);
    let points = qdrant
        .count_points(&qdrant_collection)
        .await
        .map_err(Error::Qdrant)?;
    let chunk_count = u64::try_from(chunks.len()).unwrap_or(u64::MAX);
    if generation.point_count != Some(points) || points != chunk_count {
        findings.push(format!(
            "point count mismatch: Qdrant has {points}, generation records {}, and \
             chunk set {} has {chunk_count} chunks",
            generation
                .point_count
                .map_or_else(|| "none".to_owned(), |count| count.to_string()),
            set.id
        ));
    }
    let alias = alias_name(&generation);
    let alias_target = qdrant.alias_target(&alias).await.map_err(Error::Qdrant)?;
    if alias_target.as_deref() != Some(qdrant_collection.as_str()) {
        findings.push(format!(
            "alias {alias} names {}, not generation collection {qdrant_collection}",
            alias_target.as_deref().unwrap_or("no collection")
        ));
    }
    Ok(Verification {
        collection: generation.collection_id,
        generation: generation.id,
        chunk_set: set.id,
        point_count: points,
        findings,
    })
}
