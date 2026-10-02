//! CLI composition root for the process's immutable preferences snapshot.

use super::trust;
use crate::{failure::Failure, settings::Session};
use maestro_catalog::{
    limits::Limits,
    policy::workspace::{CheckedTrust, JournalTrust},
    settings::{NoWorkspaceTrust, SessionPreferences},
};
use maestro_kernel::{
    paths::{self, Environment},
    store::Database,
    workspace::WorkspaceAuthority,
};
use std::{
    collections::BTreeSet,
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

/// Init reads its own user/root preferences without discovering or admitting a lock.
pub(super) fn init_preferences() -> Result<SessionPreferences, Failure> {
    let root = env::current_dir().map_err(|error| Failure::failed_by(&error))?;
    let database = trust::existing_database()?;
    let adapter = JournalTrust::optional(
        database
            .as_ref()
            .map(|database| database as &dyn WorkspaceAuthority),
    );
    let snapshot = SessionPreferences::load_for_init(
        &config_dir()?,
        Some(&root),
        env::home_dir().as_deref(),
        &adapter,
        &Limits::PRODUCTION,
    )
    .map_err(Failure::refused)?;
    if let Some(note) = &snapshot.discovery.note {
        eprintln!("{note}");
    }
    for skipped in &snapshot.discovery.skipped {
        eprintln!("{skipped}");
    }
    Ok(snapshot)
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

/// No-authority fallback: preferences only; pending lock registries fail closed.
pub(crate) fn at(
    config_dir: &Path,
    start: Option<&Path>,
    home: Option<&Path>,
    flags: &[String],
) -> Result<Session, Failure> {
    let snapshot = SessionPreferences::load(
        config_dir,
        start,
        home,
        &NoWorkspaceTrust,
        &Limits::PRODUCTION,
    )
    .map_err(Failure::refused)?;
    from_snapshot(config_dir, &snapshot, flags)
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
    trust: &CheckedTrust<'_>,
) -> Result<Session, Failure> {
    let compiled = compiled_backends();
    let snapshot = SessionPreferences::load(config_dir, start, home, trust, &Limits::PRODUCTION)
        .and_then(|snapshot| snapshot.admit_defaults(trust, &compiled, &Limits::PRODUCTION))
        .map_err(Failure::refused)?;
    from_snapshot(config_dir, &snapshot, flags)
}

/// Both authority paths enforce availability after the four layers resolve.
fn from_snapshot(
    config_dir: &Path,
    snapshot: &SessionPreferences,
    flags: &[String],
) -> Result<Session, Failure> {
    let compiled = compiled_backends();
    let session =
        Session::from_preferences(config_dir, snapshot, snapshot.discovery.clone(), flags)?;
    if session
        .resolved()
        .text("graph.engine")
        .is_some_and(|kind| kind != "none" && !compiled.contains(kind))
    {
        return Err(Failure::refused(
            "backend adapter is not compiled into this build",
        ));
    }
    Ok(session)
}

/// The kernel's configuration directory, which holds the user file.
fn config_dir() -> Result<PathBuf, Failure> {
    paths::config_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))
}

/// Existing S1 vector/MCP adapters linked by this composition root.
/// S3 has no qualified native graph adapter; explicit `none` needs no adapter.
pub(super) fn compiled_backends() -> BTreeSet<String> {
    BTreeSet::from(["qdrant".to_owned(), "knowledge".to_owned()])
}
