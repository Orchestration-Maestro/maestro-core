//! Why the kernel refused to record a document or a revision.

use crate::store;
use std::{error, fmt};

/// Why the kernel refused to record a document or a revision.
#[derive(Debug)]
pub enum Error {
    /// The document of this id is recorded already under another collection,
    /// source or source reference; its record stays as it is.
    DocumentConflict(String),
    /// The revision of this id is recorded already with other content: an id
    /// names one exact version of a document, so its record stays as it is.
    RevisionConflict(String),
    /// The kernel's database refused; the message and the source are its own.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentConflict(id) => write!(
                formatter,
                "the document {id} is recorded already under another collection, source or \
                 source reference"
            ),
            Self::RevisionConflict(id) => write!(
                formatter,
                "the revision {id} is recorded already with other content"
            ),
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(error) => error.source(),
            Self::DocumentConflict(_) | Self::RevisionConflict(_) => None,
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
