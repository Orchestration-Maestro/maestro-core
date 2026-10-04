//! Content-free policy failure codes: no source bytes or paths in diagnostics.
use std::{error::Error, fmt};

/// A policy cannot admit effects. No fallback is permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Invalid version, shape, bounded field or contradictory selection.
    Invalid,
    /// A required immutable resource is missing or disabled.
    Missing,
    /// Bytes, identity or admission evidence do not bind the exact reference.
    Digest,
    /// Reviewed evidence for this platform/capability is absent.
    Unqualified,
    /// The cumulative local IPC budget is exhausted.
    Deadline,
    /// Collection visibility/scopes do not permit this principal's read.
    Access,
    /// Old S1-only declaration, or an explicit null acquisition link.
    ImportOnly,
    /// Execution belongs to a later task and cannot be guessed here.
    Unsupported,
}
impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source policy refused: {self:?}")
    }
}
impl Error for Refusal {}

#[cfg(test)]
mod mutation_tests {
    #[test]
    fn s6t_refusal_display_preserves_code() {
        assert_eq!(
            super::Refusal::Digest.to_string(),
            "source policy refused: Digest"
        );
    }
}
