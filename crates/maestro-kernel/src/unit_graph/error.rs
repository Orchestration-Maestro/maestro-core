//! Refusals at the graph wire and persistence boundary.

use crate::store;
use std::{error, fmt};

/// A graph or representation was not safe to persist or expose.
#[derive(Debug)]
pub enum Error {
    /// Invalid wire syntax or noncanonical encoding.
    Json(serde_json::Error),
    /// A contract invariant was not met.
    Invalid(&'static str),
    /// The scoped record is absent or not authorized.
    NotFound,
    /// An immutable identity already names different content.
    Conflict,
    /// Database or artifact failure.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "graph JSON: {error}"),
            Self::Invalid(reason) => write!(formatter, "invalid graph: {reason}"),
            Self::NotFound => formatter.write_str("scoped graph record not found"),
            Self::Conflict => formatter.write_str("immutable graph identity conflict"),
            Self::Store(error) => error.fmt(formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Invalid(_) | Self::NotFound | Self::Conflict => None,
        }
    }
}

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

/// Refuses an unmet contract without silently repairing input.
pub(crate) fn require(condition: bool, reason: &'static str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::Invalid(reason))
    }
}
