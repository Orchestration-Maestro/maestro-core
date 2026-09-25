//! The refusal every mapping check returns.
use crate::error::Error;

/// The refusal for text that cannot be mapped back to its source.
pub(super) fn invalid_mapping() -> Error {
    Error("invalid canonical text/source mapping".into())
}
