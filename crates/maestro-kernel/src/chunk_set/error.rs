//! Why the kernel refused to begin, move or fill a chunk set.

use super::state::ChunkSetState;
use crate::store;
use std::{error, fmt};

/// Why the kernel refused to begin, move or fill a chunk set.
#[derive(Debug)]
pub enum Error {
    /// No chunk set of this id is recorded.
    UnknownChunkSet(String),
    /// The chunk set of this id is recorded already for another collection,
    /// chunk profile or counter: its record stays as it is.
    Conflict(String),
    /// The move is not one the chunk set's state allows; the chunk set stays
    /// as it is.
    IllegalMove {
        /// The chunk set's id.
        chunk_set: String,
        /// The state it is in.
        from: ChunkSetState,
        /// The state the move would have put it in.
        to: ChunkSetState,
    },
    /// The chunk set is complete or failed, so it takes no more chunks.
    NotBuilding {
        /// The chunk set's id.
        chunk_set: String,
        /// The state it is in.
        state: ChunkSetState,
    },
    /// The chunks given are not the ones the chunk set holds for the
    /// revision, or not all of that revision: nothing is recorded.
    ChunksConflict {
        /// The chunk set's id.
        chunk_set: String,
        /// The revision whose chunks they are.
        revision: String,
    },
    /// The kernel's database refused; the message and the source are its own.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownChunkSet(id) => write!(formatter, "no chunk set {id} is recorded"),
            Self::Conflict(id) => write!(
                formatter,
                "the chunk set {id} is recorded already for another collection, chunk profile \
                 or counter"
            ),
            Self::IllegalMove {
                chunk_set,
                from,
                to,
            } => write!(
                formatter,
                "the chunk set {chunk_set} cannot move from {from} to {to}: a chunk set moves \
                 only from building to complete or to failed"
            ),
            Self::NotBuilding { chunk_set, state } => write!(
                formatter,
                "the chunk set {chunk_set} is {state}: it takes no more chunks"
            ),
            Self::ChunksConflict {
                chunk_set,
                revision,
            } => write!(
                formatter,
                "the chunk set {chunk_set} holds other chunks of the revision {revision} already"
            ),
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(error) => error.source(),
            Self::UnknownChunkSet(_)
            | Self::Conflict(_)
            | Self::IllegalMove { .. }
            | Self::NotBuilding { .. }
            | Self::ChunksConflict { .. } => None,
        }
    }
}

impl From<store::Error> for Error {
    fn from(error: store::Error) -> Self {
        Self::Store(error)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}
