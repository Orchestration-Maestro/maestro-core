//! Why the gate stopped: before any work, for a collection its caller cannot
//! read, or part way, for a kernel that failed or an artifact it cannot give
//! back. What it decided before it stopped stays decided, so a rerun
//! continues where it stopped.

use maestro_kernel::{document, store};
use std::{error, fmt};

/// Why the gate stopped.
#[derive(Debug)]
pub enum Error {
    /// The caller reads no collection of this ID: it is not recorded, or no
    /// grant of the caller covers its scope. Nothing is decided.
    UnknownCollection(String),
    /// The kernel failed to read a revision, a document or a disposition, or
    /// to record one.
    Records(document::Error),
    /// The artifact store failed to give back a revision's canonical document
    /// or its original Markdown.
    Artifacts(store::Error),
    /// The canonical artifact of this revision is not a canonical document.
    Canonical {
        /// The revision.
        revision_id: String,
        /// What is wrong with its artifact.
        error: serde_json::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCollection(id) => write!(
                formatter,
                "the caller reads no collection `{id}`: it is not recorded, or no grant covers \
                 its scope"
            ),
            Self::Records(error) => write!(formatter, "the kernel's records failed: {error}"),
            Self::Artifacts(error) => write!(
                formatter,
                "the artifact store failed to give back a revision's canonical document or \
                 original: {error}"
            ),
            Self::Canonical { revision_id, error } => write!(
                formatter,
                "the canonical artifact of the revision {revision_id} is not a canonical \
                 document: {error}"
            ),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::UnknownCollection(_) => None,
            Self::Records(error) => Some(error),
            Self::Artifacts(error) => Some(error),
            Self::Canonical { error, .. } => Some(error),
        }
    }
}
