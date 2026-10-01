//! Errors from exact, scoped search storage and reads.

use crate::store;
use std::{error, fmt};

/// Why the kernel could not perform a search operation.
#[derive(Debug)]
pub enum Error {
    /// The kernel store failed; its source says why.
    Store(store::Error),
    /// The record is missing or outside the caller's scope.
    UnknownOrInaccessible,
    /// The generation has no complete search projection.
    ProjectionMissing,
    /// The generation uses a different identifier profile.
    ProfileMismatch {
        /// The profile the caller requires.
        expected: String,
        /// The profile the projection records.
        found: String,
    },
    /// The operation's input does not agree with its recorded source.
    InvalidInput(String),
    /// An existing prepared input differs from the retry.
    InputConflict,
    /// An existing member list differs from the retry.
    MembershipConflict,
    /// The result or input exceeds its fixed bound.
    TooLarge,
    /// A caller cancelled the controlled read.
    Cancelled,
    /// The controlled read reached its absolute deadline.
    TimedOut,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => fmt::Display::fmt(error, formatter),
            Self::UnknownOrInaccessible => {
                formatter.write_str("record is unknown or outside the granted scope")
            }
            Self::ProjectionMissing => formatter.write_str("search projection is not ready"),
            Self::ProfileMismatch { expected, found } => write!(
                formatter,
                "search projection profile {found:?} does not match {expected:?}"
            ),
            Self::InvalidInput(reason) => formatter.write_str(reason),
            Self::InputConflict => {
                formatter.write_str("prepared input conflicts with its recorded value")
            }
            Self::MembershipConflict => {
                formatter.write_str("chunk-set members conflict with the recorded list")
            }
            Self::TooLarge => formatter.write_str("search result exceeds its fixed bound"),
            Self::Cancelled => formatter.write_str("search read was cancelled"),
            Self::TimedOut => formatter.write_str("search read reached its deadline"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::UnknownOrInaccessible
            | Self::ProjectionMissing
            | Self::ProfileMismatch { .. }
            | Self::InvalidInput(_)
            | Self::InputConflict
            | Self::MembershipConflict
            | Self::TooLarge
            | Self::Cancelled
            | Self::TimedOut => None,
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
        Self::Store(store::Error::Sqlite(error))
    }
}
