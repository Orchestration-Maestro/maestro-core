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

/// The data directory entries the kernel does not own and the database
/// creation temporaries, each in path order. The database, its WAL and SHM,
/// artifact tree and search service are owned; temporary links are only listed.
pub(super) fn directory_findings(data: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(data) else {
        return (Vec::new(), Vec::new());
    };
    let mut foreign = Vec::new();
    let mut database_temporaries = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            foreign.push(path);
            continue;
        };
        if database_temporary(name) {
            database_temporaries.push(path);
        } else if !owned(name) {
            foreign.push(path);
        }
    }
    foreign.sort();
    database_temporaries.sort();
    (foreign, database_temporaries)
}

/// Whether `name` is a leftover `<database>.tmp-<process>-<number>` link.
fn database_temporary(name: &str) -> bool {
    let Some((process, number)) = name
        .strip_prefix(DATABASE)
        .and_then(|suffix| suffix.strip_prefix(".tmp-"))
        .and_then(|suffix| suffix.split_once('-'))
    else {
        return false;
    };
    [process, number]
        .into_iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Whether the kernel owns the entry `name` of its data directory.
fn owned(name: &str) -> bool {
    name == DATABASE
        || matches!(name.strip_prefix(DATABASE), Some("-wal" | "-shm"))
        || name == ARTIFACTS
        || name == SEARCH_SERVICE
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
