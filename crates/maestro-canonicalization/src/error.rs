//! The crate's one error type: a refusal that says why, never the source text.
use std::{error, fmt};

/// An input or execution error that prevents canonicalization.
#[derive(Debug)]
pub struct Error(
    /// Human-readable refusal, without source-document contents.
    pub String,
);
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl error::Error for Error {}
