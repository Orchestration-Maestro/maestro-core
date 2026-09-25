//! The refusal every structural check returns.
use crate::error::Error;

/// The refusal when a chunk's context or structure cannot be represented within the measured
/// profile.
pub(super) fn structure_error() -> Error {
    Error("chunk context/structure cannot be represented under the measured profile".into())
}
