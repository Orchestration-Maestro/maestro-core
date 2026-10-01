//! Representation-set boundary errors.
use crate::{store, unit_graph::Error as GraphError};
use std::{error, fmt};

/// Representation metadata, a shard, or its persistence record was invalid.
#[derive(Debug)]
pub enum Error {
    /// Invalid canonical JSON.
    Json(serde_json::Error),
    /// Database or artifact failure.
    Store(store::Error),
    /// Recorded graph failed its independent validation.
    Graph(GraphError),
    /// Invalid persisted or supplied representation data.
    Invalid(&'static str),
    /// Scoped record is absent or unauthorized.
    NotFound,
    /// Immutable identity conflict or illegal state transition.
    Conflict,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "representation JSON: {error}"),
            Self::Store(error) => error.fmt(f),
            Self::Graph(error) => error.fmt(f),
            Self::Invalid(reason) => write!(f, "invalid representation: {reason}"),
            Self::NotFound => f.write_str("scoped representation not found"),
            Self::Conflict => f.write_str("representation identity conflict"),
        }
    }
}
impl error::Error for Error {}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
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
impl From<GraphError> for Error {
    fn from(error: GraphError) -> Self {
        Self::Graph(error)
    }
}

/// Refuses an unmet representation contract.
pub(super) fn require(condition: bool, reason: &'static str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::Invalid(reason))
    }
}
