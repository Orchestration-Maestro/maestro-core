//! CLI composition root for the process's immutable preferences snapshot.

use super::trust;
use crate::{failure::Failure, settings::Session};
use maestro_catalog::{
    limits::Limits,
    policy::workspace::{CheckedTrust, JournalTrust, WorkspaceTrust},
    settings::{NoWorkspaceTrust, SessionPreferences},
};
use maestro_kernel::{
    paths::{self, Environment},
    store::Database,
};
use std::{
    env,
    path::{Path, PathBuf},
};

/// The command line's session: the user file, the project file found
/// from the working directory, and `flags`.
///
/// # Errors
///
/// [`Failure::Refused`] naming a file or flag that is refused, and
/// [`Failure::Failed`] when the configuration directory cannot be found.
pub(crate) fn for_cli(flags: &[String]) -> Result<Session, Failure> {
    let start = env::current_dir().ok();
    current_at(
        &config_dir()?,
        start.as_deref(),
        env::home_dir().as_deref(),
        flags,
    )
}

/// The MCP server's session: the user file, the project file found from
/// `workspace` when one is given, and `flags`.
///
/// # Errors
///
/// As [`for_cli`], and [`Failure::Refused`] for a workspace
/// that is not a directory. Unsafe or external candidates warn and fall back.
pub(crate) fn for_mcp(workspace: Option<&Path>, flags: &[String]) -> Result<Session, Failure> {
    if let Some(workspace) = workspace
        && !workspace.is_dir()
    {
        return Err(Failure::refused(format!(
            "--workspace {}: the path is not a directory: no project file is read",
            workspace.display()
        )));
    }
    current_at(&config_dir()?, workspace, env::home_dir().as_deref(), flags)
}

/// [`for_mcp`] with the directories given.
#[cfg(test)]
pub(crate) fn for_mcp_at(
    config_dir: &Path,
    workspace: Option<&Path>,
    home: Option<&Path>,
    flags: &[String],
) -> Result<Session, Failure> {
    if let Some(workspace) = workspace
        && !workspace.is_dir()
    {
        return Err(Failure::refused(format!(
            "--workspace {}: the path is not a directory: no project file is read",
            workspace.display()
        )));
    }
    at(config_dir, workspace, home, flags)
}

/// Load the file-backed source and pass its discovery metadata to the session.
pub(crate) fn at(
    config_dir: &Path,
    start: Option<&Path>,
    home: Option<&Path>,
    flags: &[String],
) -> Result<Session, Failure> {
    at_with_trust(config_dir, start, home, flags, &NoWorkspaceTrust)
}

/// Production sessions consult kernel authority without creating a fresh database.
fn current_at(
    config_dir: &Path,
    start: Option<&Path>,
    home: Option<&Path>,
    flags: &[String],
) -> Result<Session, Failure> {
    let data =
        paths::data_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))?;
    let Ok(database) = Database::open_read_only_in(&data) else {
        return at(config_dir, start, home, flags);
    };
    let boundaries = trust::boundaries()?;
    let adapter = JournalTrust::new(&database);
    at_with_trust(
        config_dir,
        start,
        home,
        flags,
        &CheckedTrust::new(&adapter, &boundaries),
    )
}

/// Common storage-independent discovery path; injected tests need no process authority.
fn at_with_trust(
    config_dir: &Path,
    start: Option<&Path>,
    home: Option<&Path>,
    flags: &[String],
    trust: &dyn WorkspaceTrust,
) -> Result<Session, Failure> {
    let snapshot = SessionPreferences::load(config_dir, start, home, trust, &Limits::PRODUCTION)
        .map_err(Failure::refused)?;
    Session::from_preferences(config_dir, &snapshot, snapshot.discovery.clone(), flags)
}

/// The kernel's configuration directory, which holds the user file.
fn config_dir() -> Result<PathBuf, Failure> {
    paths::config_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))
}
