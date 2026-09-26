//! The rights a grant gives on a scope. S1 has one, read; a later right adds
//! rows to the `grants` table, not columns.

/// A right a grant gives on a scope and every scope below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Right {
    /// Reading what the scope holds.
    Read,
}

impl Right {
    /// Its name, as the `right` column of `grants` holds it.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
        }
    }
}
