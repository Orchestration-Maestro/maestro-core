//! Why the kernel refused to record or read an evaluation report.

use crate::store;
use std::{error, fmt};

/// Why the kernel refused to record or read an evaluation report.
#[derive(Debug)]
pub enum Error {
    /// The collection records no generation of this id: a report measures a
    /// generation of its own collection. Nothing is recorded.
    UnknownGeneration {
        /// The collection the report names.
        collection: String,
        /// The generation it names.
        generation: i64,
    },
    /// The kernel's database refused; the message and the source are its own.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownGeneration {
                collection,
                generation,
            } => write!(
                formatter,
                "the collection {collection} records no generation {generation}"
            ),
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(error) => error.source(),
            Self::UnknownGeneration { .. } => None,
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
