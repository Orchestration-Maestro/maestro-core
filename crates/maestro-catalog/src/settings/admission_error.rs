//! Typed availability refusals; ordinary admission errors retain their existing diagnostics.
use std::{error, fmt};

/// Read-only authoring-lock admission failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    /// A selected base adapter is unavailable in this binary.
    BackendNotCompiled {
        /// Manifest-declared adapter name, not an engine path.
        backend: String,
        /// Existing named lock diagnostic and recovery, preserved verbatim.
        message: String,
    },
    /// Any existing trust, ownership, shape or bounds refusal.
    Refused(String),
}

impl From<String> for AdmissionError {
    fn from(message: String) -> Self {
        Self::Refused(message)
    }
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendNotCompiled { message, .. } | Self::Refused(message) => {
                formatter.write_str(message)
            }
        }
    }
}
impl error::Error for AdmissionError {}
