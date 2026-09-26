//! Why a preparation stopped: before any work, for a collection its caller
//! cannot read whole, or a chunk set that failed before; part way, for a
//! counter that refused, a kernel that failed, or a caller that stopped it.
//! What it recorded before it stopped stays, so a rerun resumes its chunk
//! set, unless the counter changed under its card, which fails the set.

use super::error::TokenizerError;
use maestro_kernel::{chunk_set, document, store};
use std::{error, fmt};

/// Why a preparation stopped.
#[derive(Debug)]
pub enum Error {
    /// The caller's scopes do not cover this scope of the collection, whose
    /// revisions the preparation chunks: nothing is prepared.
    NotVisible(String),
    /// The chunk set of this id failed before, which is final: nothing is
    /// prepared.
    Failed(String),
    /// The counter refused. When its canaries changed under its card, the
    /// chunk set failed with it; otherwise, such as a router with no free
    /// room, the set stays building and a rerun resumes it.
    Counter(TokenizerError),
    /// The caller's observer stopped the preparation, as a job does once its
    /// lease is lost: the chunk set stays building.
    Stopped,
    /// The kernel failed to read the collection's revisions and documents,
    /// or to record their occurrences and near duplicates.
    Records(document::Error),
    /// The kernel failed to begin, fill or end the chunk set.
    ChunkSet(chunk_set::Error),
    /// The kernel failed to read or store an artifact.
    Artifacts(store::Error),
    /// The chunk set's manifest could not be written or read back.
    Manifest(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotVisible(scope) => write!(
                formatter,
                "the caller cannot read the scope `{scope}`, whose revisions the preparation \
                 chunks: grant it first"
            ),
            Self::Failed(id) => write!(
                formatter,
                "the chunk set {id} failed, which is final: its counts were not trusted; a new \
                 model card makes a new set"
            ),
            Self::Counter(error) => write!(formatter, "the counter refused: {error}"),
            Self::Stopped => formatter.write_str(
                "the preparation was stopped by its caller: its chunk set stays building, and a \
                 rerun resumes it",
            ),
            Self::Records(error) => write!(formatter, "the kernel's records failed: {error}"),
            Self::ChunkSet(error) => write!(formatter, "the chunk set failed: {error}"),
            Self::Artifacts(error) => write!(formatter, "the artifact store failed: {error}"),
            Self::Manifest(error) => {
                write!(formatter, "the chunk set's manifest is not valid: {error}")
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Counter(error) => Some(error),
            Self::Records(error) => Some(error),
            Self::ChunkSet(error) => Some(error),
            Self::Artifacts(error) => Some(error),
            Self::Manifest(error) => Some(error),
            Self::NotVisible(_) | Self::Failed(_) | Self::Stopped => None,
        }
    }
}
