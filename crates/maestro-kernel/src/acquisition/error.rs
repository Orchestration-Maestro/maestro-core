//! Refusals from the frontier port; no refused operation acknowledges work.
use crate::{job, store};
use std::{error, fmt};

/// Why an acquisition operation could not commit.
#[derive(Debug)]
pub enum Error {
    /// An identity, lease term or page bound is invalid.
    Invalid,
    /// A handle is unknown, expired, forged or fenced by a newer epoch.
    Lost,
    /// Work is already leased, captured, or has exhausted its attempt bound.
    Unavailable,
    /// An idempotent acknowledgement tried to substitute another artifact.
    Conflict,
    /// The existing job machinery refused ownership.
    Job(job::Error),
    /// The kernel store refused the transaction or artifact.
    Store(store::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => f.write_str("invalid frontier identity or bound"),
            Self::Lost => f.write_str("frontier lease is unknown, expired or fenced"),
            Self::Unavailable => f.write_str("frontier work is unavailable"),
            Self::Conflict => f.write_str("frontier acknowledgement conflicts with its capture"),
            Self::Job(error) => fmt::Display::fmt(error, f),
            Self::Store(error) => fmt::Display::fmt(error, f),
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Job(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Invalid | Self::Lost | Self::Unavailable | Self::Conflict => None,
        }
    }
}
impl From<job::Error> for Error {
    fn from(error: job::Error) -> Self {
        Self::Job(error)
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
