//! Why the kernel refused to create or move a generation.

use super::state::GenerationState;
use crate::store;
use std::{error, fmt};

/// Why the kernel refused to create or move a generation.
#[derive(Debug)]
pub enum Error {
    /// No generation of this id is recorded.
    UnknownGeneration(i64),
    /// The move is not the one the generation's state allows; the generation
    /// stays as it is.
    IllegalMove {
        /// The generation's id.
        generation: i64,
        /// The state it is in.
        from: GenerationState,
        /// The state the move would have put it in.
        to: GenerationState,
    },
    /// The kernel's database refused; the message and the source are its own.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownGeneration(id) => write!(formatter, "no generation {id} is recorded"),
            Self::IllegalMove {
                generation,
                from,
                to,
            } => write!(
                formatter,
                "generation {generation} cannot move from {from} to {to}: a generation moves \
                 only from building to verified, then to published, then to retired"
            ),
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(error) => error.source(),
            Self::UnknownGeneration(_) | Self::IllegalMove { .. } => None,
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
