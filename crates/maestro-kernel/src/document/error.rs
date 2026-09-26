//! Why the kernel refused to record a collection, a source, a document or a
//! revision.

use crate::{scope::InvalidName, store};
use std::{error, fmt};

/// Why the kernel refused to record a collection, a source, a document or a
/// revision.
#[derive(Debug)]
pub enum Error {
    /// A collection or source id is not a scope name, so it would not form
    /// one segment of its scope's path: nothing is recorded.
    InvalidId(InvalidName),
    /// The document of this id is recorded already under another collection,
    /// source or source reference; its record stays as it is.
    DocumentConflict(String),
    /// The revision of this id is recorded already with other content: an id
    /// names one exact version of a document, so its record stays as it is.
    RevisionConflict(String),
    /// Another document of the collection is recorded already from the same
    /// source reference: a source reference names one document in each
    /// collection, so the record stays as it is.
    SourceRefConflict {
        /// The document recorded from the source reference.
        recorded: String,
        /// The document refused.
        given: String,
    },
    /// The kernel's database refused; the message and the source are its own.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(invalid) => write!(formatter, "the id is refused: {invalid}"),
            Self::DocumentConflict(id) => write!(
                formatter,
                "the document {id} is recorded already under another collection, source or \
                 source reference"
            ),
            Self::RevisionConflict(id) => write!(
                formatter,
                "the revision {id} is recorded already with other content"
            ),
            Self::SourceRefConflict { recorded, given } => write!(
                formatter,
                "the document {given} is refused: its collection records the document \
                 {recorded} from the same source reference"
            ),
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::InvalidId(invalid) => Some(invalid),
            Self::Store(error) => error.source(),
            Self::DocumentConflict(_)
            | Self::RevisionConflict(_)
            | Self::SourceRefConflict { .. } => None,
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
