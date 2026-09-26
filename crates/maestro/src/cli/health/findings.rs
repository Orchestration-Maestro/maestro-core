//! What doctor finds but must not touch: the entries of the kernel's data
//! directory the kernel does not own, such as the files maestro v1 left
//! there (`ledger.sqlite3`, `material/`, `maestro.sock`), and the grants of
//! `config.toml` that reach no scope the kernel knows. Doctor lists them,
//! and never changes or removes them.

use super::kernel::{ARTIFACTS, DATABASE};
use maestro_kernel::{
    scope::{Scope, ScopeSet},
    store::{self, Database},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The directory setup installs the search service in, in the data
/// directory.
const SEARCH_SERVICE: &str = "qdrant";

/// The entries of the data directory `data` the kernel does not own, in
/// path order: the kernel owns its database with the files SQLite keeps
/// beside it, its artifact tree, and the search service setup installs.
pub(super) fn foreign_entries(data: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(data) else {
        return Vec::new();
    };
    let mut foreign: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| !entry.file_name().to_str().is_some_and(owned))
        .map(|entry| entry.path())
        .collect();
    foreign.sort();
    foreign
}

/// Whether the kernel owns the entry `name` of its data directory.
fn owned(name: &str) -> bool {
    name.starts_with(DATABASE) || name == ARTIFACTS || name == SEARCH_SERVICE
}

/// The grants of `scopes` that reach no scope the kernel knows, in path
/// order: neither its workspace nor any collection or source it records.
///
/// # Errors
///
/// [`store::Error`] when `database` cannot be read.
pub(super) fn unreached_grants(
    database: &Database,
    scopes: &ScopeSet,
) -> Result<Vec<Scope>, store::Error> {
    let known = database.known_scopes(scopes)?;
    Ok(scopes
        .granted()
        .filter(|grant| !known.iter().any(|scope| grant.covers(scope)))
        .cloned()
        .collect())
}
