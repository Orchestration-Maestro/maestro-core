//! Read and validate write-ahead journals through the shared held-handle filesystem.
use super::effects;
use super::{
    names::journal_name,
    plan::{FilePlan, PlannedFile, validate_id, validate_plan},
};
use crate::policy::workspace::CheckedTrust;
use std::{io, path::Path, str};

/// Serialized form of one write-ahead file plan.
#[derive(serde::Deserialize)]
struct Journal {
    /// The stable digest identifying the plan.
    id: String,
    /// The intended files and their content digests.
    entries: Vec<PlannedFile>,
}

/// Encode one internal state record as TOML bytes.
pub(super) fn record_bytes<T: serde::Serialize>(value: &T) -> io::Result<Vec<u8>> {
    toml::to_string(value)
        .map(String::into_bytes)
        .map_err(|error| io::Error::other(format!("cannot encode file state: {error}")))
}

/// Read a regular state file, distinguishing absence from every other failure.
pub(super) fn read_optional(
    root: &Path,
    name: &str,
    trust: &CheckedTrust<'_>,
) -> io::Result<Option<Vec<u8>>> {
    match effects::read(root, name, trust) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Read and validate the write-ahead journal for the exact plan identified by `id`.
///
/// # Errors
/// Returns an error for absent, malformed, or mismatched journal contents.
pub(super) fn read_journal(
    root: &Path,
    id: &str,
    trust: &CheckedTrust<'_>,
) -> io::Result<FilePlan> {
    validate_id(id)?;
    let bytes = effects::read(root, &format!(".maestro-files/{}", journal_name(id)), trust)
        .map_err(|error| {
            io::Error::new(error.kind(), format!("no recoverable file plan: {error}"))
        })?;
    let journal: Journal = toml::from_str(str::from_utf8(&bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid file journal: {error}"),
        )
    })?)
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid file journal: {error}"),
        )
    })?;
    let plan = FilePlan {
        id: journal.id,
        entries: journal.entries,
        applied: false,
    };
    if plan.id != id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "journal plan identity mismatch",
        ));
    }
    validate_plan(&plan)?;
    Ok(plan)
}
