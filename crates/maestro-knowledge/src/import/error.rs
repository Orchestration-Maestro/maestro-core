//! Why an import stopped: before any work, for a manifest it cannot find or
//! a scope its caller cannot read, or part way, for a manifest it cannot
//! read, a kernel that failed or a caller that stopped it. What it recorded
//! before it stopped stays, so a rerun continues where it stopped.

use maestro_kernel::{binding, document, journal, store};
use std::{error, fmt, io};

/// Why an import stopped.
#[derive(Debug)]
pub enum Error {
    /// Nothing binds the binding of a source's manifest: nothing is
    /// imported.
    Binding(binding::Error),
    /// The caller's scopes do not cover this scope of a source, whose records
    /// the import reads and writes: nothing is imported.
    NotVisible(String),
    /// The manifest of this source cannot be read.
    Manifest {
        /// The source whose manifest it is.
        source_id: String,
        /// What the operating system reported.
        error: io::Error,
    },
    /// The kernel failed to read or record a collection, a source, a
    /// document, a revision or a disposition.
    Records(document::Error),
    /// The kernel failed to store an artifact.
    Artifacts(store::Error),
    /// The journal failed to record the import's completion.
    Journal(journal::Error),
    /// The caller's observer stopped the import
    /// ([`import_observed`](super::import_observed)), as a job does once its
    /// lease is lost: what was recorded before stays, and no completion is
    /// journaled.
    Stopped,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(error) => write!(formatter, "a manifest cannot be found: {error}"),
            Self::NotVisible(scope) => write!(
                formatter,
                "the caller cannot read the scope `{scope}`, whose records the import writes: \
                 grant it first"
            ),
            Self::Manifest { source_id, error } => write!(
                formatter,
                "the manifest of the source `{source_id}` cannot be read: {error}"
            ),
            Self::Records(error) => write!(formatter, "the kernel's records failed: {error}"),
            Self::Artifacts(error) => write!(formatter, "the artifact store failed: {error}"),
            Self::Journal(error) => write!(formatter, "the journal failed: {error}"),
            Self::Stopped => formatter.write_str(
                "the import was stopped by its caller: what it recorded stays, and a rerun \
                 continues where it stopped",
            ),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Binding(error) => Some(error),
            Self::NotVisible(_) | Self::Stopped => None,
            Self::Manifest { error, .. } => Some(error),
            Self::Records(error) => Some(error),
            Self::Artifacts(error) => Some(error),
            Self::Journal(error) => Some(error),
        }
    }
}
