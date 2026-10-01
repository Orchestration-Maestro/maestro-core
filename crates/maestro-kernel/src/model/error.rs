//! Why the kernel refused a model-card, evaluation, or selection operation.

use crate::{gateway::card_types::CardError, store};
use std::{error, fmt};

/// Why a model-registry operation failed.
#[derive(Debug)]
pub enum Error {
    /// The request lacks a collection scope or names an invalid collection.
    Unauthorized,
    /// Request fields conflict with immutable registered metadata.
    Invalid(String),
    /// The card artifact is malformed or is a legacy v1 card.
    Card(CardError),
    /// A stored artifact and its row metadata disagree or fail verification.
    Integrity(String),
    /// The database or its artifact store refused the operation.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => {
                formatter.write_str("the collection is not writable in this scope")
            }
            Self::Invalid(reason) => write!(formatter, "invalid model-registry record: {reason}"),
            Self::Card(error) => fmt::Display::fmt(error, formatter),
            Self::Integrity(reason) => {
                write!(formatter, "model-registry integrity check failed: {reason}")
            }
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Card(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Unauthorized | Self::Invalid(_) | Self::Integrity(_) => None,
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

impl From<CardError> for Error {
    fn from(error: CardError) -> Self {
        Self::Card(error)
    }
}
