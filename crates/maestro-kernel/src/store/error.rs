//! Why the kernel's database refused an operation.

use crate::artifact::{self, Digest};
use std::{error, fmt, io, path::PathBuf};

/// Why the kernel's database refused an operation.
#[derive(Debug)]
pub enum Error {
    /// The database records a migration this binary does not carry: a newer
    /// binary migrated it, and this one would misread its tables.
    UnknownMigration(String),
    /// No artifact is recorded under the digest.
    UnknownArtifact(Digest),
    /// The artifact has no pin to remove.
    NotPinned(Digest),
    /// The bytes are recorded already, under another media type; the record
    /// stays as it is.
    MediaConflict {
        /// The digest of the bytes.
        digest: Digest,
        /// The media type the database records.
        recorded: String,
        /// The media type the put gave.
        given: String,
    },
    /// The artifact store failed; the message and the source are its own.
    Artifact(artifact::Error),
    /// The database file or one of its directories could not be created.
    Io {
        /// The file or directory it concerned.
        path: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },
    /// SQLite refused an operation; the error's source says why.
    Sqlite(rusqlite::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMigration(name) => write!(
                formatter,
                "the kernel database records migration {name}, which this binary does not \
                 carry: a newer binary migrated it"
            ),
            Self::UnknownArtifact(digest) => write!(
                formatter,
                "no artifact is recorded under sha256:{}",
                digest.as_str()
            ),
            Self::NotPinned(digest) => write!(
                formatter,
                "the artifact sha256:{} has no pin to remove",
                digest.as_str()
            ),
            Self::MediaConflict {
                digest,
                recorded,
                given,
            } => write!(
                formatter,
                "the artifact sha256:{} is recorded as {recorded}, not {given}",
                digest.as_str()
            ),
            Self::Artifact(error) => fmt::Display::fmt(error, formatter),
            Self::Io { path, .. } => write!(formatter, "cannot access {}", path.display()),
            Self::Sqlite(_) => formatter.write_str("the kernel database refused the operation"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Artifact(error) => error.source(),
            Self::Io { source, .. } => Some(source),
            Self::Sqlite(source) => Some(source),
            Self::UnknownMigration(_)
            | Self::UnknownArtifact(_)
            | Self::NotPinned(_)
            | Self::MediaConflict { .. } => None,
        }
    }
}

impl From<artifact::Error> for Error {
    fn from(error: artifact::Error) -> Self {
        Self::Artifact(error)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}
