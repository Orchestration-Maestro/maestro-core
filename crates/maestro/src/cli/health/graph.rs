//! The embedded graph's check, which `status` and `doctor` run: the selected
//! engine, the graph's own directory, and each graph file the kernel
//! published, opened read-only, queried, closed, reopened and queried again.
//! It creates, repairs, migrates, fetches and deletes nothing, and takes no
//! path from the caller: files come only from the [`PublishedGraph`] port,
//! and each must lie inside the graph's directory, with no link on the way.

use super::check::Check;
use crate::{
    cli::setup::graph::{DIRECTORY, ENGINE_BUILT, is_private},
    failure::Failure,
    settings::{GraphEngine, Session},
};
use maestro_kernel::paths::{self, Environment};
use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path},
    time::{Duration, Instant},
};

/// The check's name, in both documents.
const NAME: &str = "graph";

/// The next action for a graph file that does not open or answer: the graph
/// is a projection, rebuilt from the kernel, so its files are never repaired.
const REBUILD: &str = "keep the files as they are, and rebuild the projection from the kernel's \
                       database and artifacts, offline";

/// The graph files the kernel's projection receipt published. G26's adapter
/// publishes none; G27's reads the receipt and owns the files' layout.
pub(super) trait PublishedGraph {
    /// What the receipt publishes.
    ///
    /// # Errors
    ///
    /// Why the receipt cannot be read.
    fn receipt(&self) -> Result<Receipt<'_>, String>;
}

/// What a projection receipt publishes.
pub(super) enum Receipt<'a> {
    /// No graph is published yet: there is nothing to open.
    NonePublished,
    /// These files, in the receipt's order.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "G27's receipt adapter publishes files; G26 tests a fake"
        )
    )]
    Files(Vec<Box<dyn PublishedFile + 'a>>),
}

/// A graph file a receipt published.
pub(super) trait PublishedFile {
    /// Its path, which the check requires inside the graph's directory.
    fn path(&self) -> &Path;

    /// Opens it read-only with the engine, following no link.
    ///
    /// # Errors
    ///
    /// The engine's message when it refuses the file.
    fn open_read_only(&self) -> Result<Box<dyn OpenGraph + '_>, String>;
}

/// A published graph file, open read-only until dropped.
pub(super) trait OpenGraph {
    /// Runs the smallest query, `RETURN 1`.
    ///
    /// # Errors
    ///
    /// The engine's message when the query fails.
    fn query_one(&self) -> Result<(), String>;
}

/// G26's receipt adapter: until G27 writes the projection receipt, no graph
/// is published.
pub(super) struct NonePublished;

impl PublishedGraph for NonePublished {
    fn receipt(&self) -> Result<Receipt<'_>, String> {
        Ok(Receipt::NonePublished)
    }
}

/// The graph's check in `environment` with the `graph.engine` of `session`.
pub(super) fn check(environment: &Environment, session: Result<Session, Failure>) -> Check {
    check_with(environment, session, ENGINE_BUILT, &NonePublished)
}

/// The graph's check, in a build that holds the engine when `engine_built`,
/// with the files `published` publishes.
pub(super) fn check_with(
    environment: &Environment,
    session: Result<Session, Failure>,
    engine_built: bool,
    published: &dyn PublishedGraph,
) -> Check {
    let directory = match paths::data_dir(environment) {
        Ok(data) => data.join(DIRECTORY),
        Err(error) => {
            return Check::failed(
                NAME,
                "data directory",
                error.to_string(),
                "set XDG_DATA_HOME to a directory of yours",
            );
        }
    };
    let target = directory.display().to_string();
    let engine = match session.and_then(|session| GraphEngine::from_session(&session)) {
        Ok(engine) => engine,
        Err(error) => {
            return Check::failed(
                NAME,
                &target,
                error.to_string(),
                "fix the settings error `maestro doctor` names under settings",
            );
        }
    };
    if engine == GraphEngine::None {
        return Check::not_checked(NAME, &target, "the graph is off (graph.engine = none)");
    }
    if !engine_built {
        return Check::failed(
            NAME,
            &target,
            "graph.engine = lbug, but this maestro was built without the engine",
            "use a maestro built with the `engine` feature, or set graph.engine to none",
        );
    }
    if let Some(problem) = directory_problem(&directory) {
        return Check::failed(NAME, &target, problem.0, problem.1);
    }
    match published.receipt() {
        Err(error) => Check::failed(
            NAME,
            &target,
            format!("the projection receipt cannot be read: {error}"),
            "check the kernel's database with `maestro doctor`, then rebuild the projection",
        ),
        Ok(Receipt::NonePublished) => Check::not_checked(
            NAME,
            &target,
            "no graph is published yet, so no file was opened",
        ),
        Ok(Receipt::Files(files)) => probe_all(&directory, &files),
    }
}

/// What is wrong with the graph's `directory`, and its next action; none
/// when it is a directory of its owner's alone.
fn directory_problem(directory: &Path) -> Option<(String, &'static str)> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_symlink() => Some((
            "the graph directory was relocated: it is a link".to_owned(),
            "stop every maestro, move what the link points to into its place, \
             then run `maestro doctor` again",
        )),
        Ok(metadata) if !metadata.is_dir() => Some((
            "the graph path is not a directory".to_owned(),
            "move it aside, then run `maestro setup --yes`",
        )),
        Ok(metadata) if is_private(&metadata) => None,
        Ok(_) => Some((
            "the graph directory's permissions let others in".to_owned(),
            "run `maestro setup --yes`, which gives it mode 0700",
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Some((
            "the graph directory is missing".to_owned(),
            "run `maestro setup --yes`, which creates it",
        )),
        Err(error) => Some((error.to_string(), "check the data directory's permissions")),
    }
}

/// Probes each of `files` inside `directory`, stopping at the first
/// problem.
fn probe_all(directory: &Path, files: &[Box<dyn PublishedFile + '_>]) -> Check {
    if files.is_empty() {
        return Check::failed(
            NAME,
            &directory.display().to_string(),
            "the projection receipt publishes no file",
            REBUILD,
        );
    }
    let mut seen = Vec::with_capacity(files.len());
    for file in files {
        let path = file.path();
        if let Err(problem) = inside(directory, path) {
            return Check::failed(
                NAME,
                &path.display().to_string(),
                problem,
                "keep the file, and rebuild the projection so its receipt names its own files",
            );
        }
        match probe(file.as_ref()) {
            Ok(detail) => seen.push(detail),
            Err(check) => return check,
        }
    }
    Check::passed(NAME, &directory.display().to_string(), seen.join("; "))
}

/// Why `path` is not a published graph file: outside `directory`, a
/// component other than a plain name, a link, or not a file.
fn inside(directory: &Path, path: &Path) -> Result<(), String> {
    let Ok(relative) = path.strip_prefix(directory) else {
        return Err("the published file lies outside the graph directory".to_owned());
    };
    let mut current = directory.to_path_buf();
    let mut last = None;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("the published file's path is not plain names".to_owned());
        };
        current.push(name);
        let metadata = fs::symlink_metadata(&current).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{} is a link", current.display()));
        }
        last = Some(metadata);
    }
    match last {
        Some(metadata) if metadata.is_file() => Ok(()),
        Some(_) => Err("the published file is not a file".to_owned()),
        None => Err("the published file is the graph directory itself".to_owned()),
    }
}

/// Opens `file` read-only and queries it, closes it, then does both again.
fn probe(file: &dyn PublishedFile) -> Result<String, Check> {
    let opened = open_and_query(file)?;
    let reopened = open_and_query(file)?;
    Ok(format!(
        "{}: opened read-only in {opened:?}, reopened in {reopened:?}, answered both times",
        file.path().display()
    ))
}

/// How long `file` took to open read-only, once it answered `RETURN 1`; it
/// is closed again on return.
fn open_and_query(file: &dyn PublishedFile) -> Result<Duration, Check> {
    let started = Instant::now();
    let open = file
        .open_read_only()
        .map_err(|message| open_failure(file.path(), &message))?;
    let opened = started.elapsed();
    open.query_one().map_err(|message| {
        Check::failed(
            NAME,
            &file.path().display().to_string(),
            format!("the graph file opens but does not answer, likely corrupt: {message}"),
            REBUILD,
        )
    })?;
    Ok(opened)
}

/// The check for a `path` the engine would not open, saying `message`: a
/// writer's lock, or a file corrupt or unreadable.
fn open_failure(path: &Path, message: &str) -> Check {
    let target = path.display().to_string();
    if message.contains("lock") {
        Check::failed(
            NAME,
            &target,
            format!("a writer holds the graph file's lock: {message}"),
            "let the writer finish, then run `maestro doctor` again",
        )
    } else {
        Check::failed(
            NAME,
            &target,
            format!("the graph file does not open read-only, corrupt or unreadable: {message}"),
            REBUILD,
        )
    }
}
