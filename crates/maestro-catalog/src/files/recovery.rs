//! Read and validate write-ahead journals through the shared held-handle filesystem.
use super::plan::{FilePlan, PlannedFile, validate_id, validate_plan};
use maestro_filesystem::Directory;
use std::{io, path::Path, str};

/// Serialized form of one write-ahead file plan.
#[derive(serde::Deserialize)]
struct Journal {
    /// The stable digest identifying the plan.
    id: String,
    /// The intended files and their content digests.
    entries: Vec<PlannedFile>,
}

/// Hidden directory for journals and ownership records.
const STATE_DIR: &str = ".maestro-files";

/// Open the private state directory through the shared held-handle implementation.
pub(super) fn state_directory(root: &Path) -> io::Result<Directory> {
    Directory::open(root, Path::new(STATE_DIR), true)
}

/// Name the write-ahead journal for one immutable plan.
pub(super) fn journal_name(id: &str) -> String {
    format!("journal-{id}.toml")
}

/// Name the committed ownership record for one immutable plan.
pub(super) fn ownership_name(id: &str) -> String {
    format!("ownership-{id}.toml")
}

/// Encode one internal state record as TOML bytes.
pub(super) fn record_bytes<T: serde::Serialize>(value: &T) -> io::Result<Vec<u8>> {
    toml::to_string(value)
        .map(String::into_bytes)
        .map_err(|error| io::Error::other(format!("cannot encode file state: {error}")))
}

/// Read a regular state file, distinguishing absence from every other failure.
pub(super) fn read_optional(directory: &Directory, name: &str) -> io::Result<Option<Vec<u8>>> {
    match directory.read_regular(name) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Read and validate the write-ahead journal for the exact plan identified by `id`.
///
/// # Errors
/// Returns an error for absent, malformed, or mismatched journal contents.
pub(super) fn read_journal(root: &Path, id: &str) -> io::Result<FilePlan> {
    validate_id(id)?;
    let state = state_directory(root)?;
    let bytes = state.read_regular(&journal_name(id)).map_err(|error| {
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
