//! `maestro setup`: the embedded graph's directory, then the search
//! service's install, each previewed, then done with `--yes`. It opens no
//! kernel and no graph: it only reads, then writes the graph's directory and
//! the service's own files and asks systemd's user manager to run it.

#[cfg(not(feature = "engine"))]
use super::graph::run as graph_setup;
#[cfg(feature = "engine")]
use super::graph_engine::run as graph_setup;
use super::{
    graph::GraphSetup,
    release::{GRPC_PORT, HOST, HTTP_PORT, Release, SERVICE, release_for},
    service::{Layout, Step, apply, survey, user_manager},
    tools::Tools,
};
use crate::{
    cli::output::{Output, diagnose},
    failure::Failure,
    settings::{GraphEngine, Session},
};
use maestro_kernel::paths::{self, Environment};
use serde::Serialize;
use std::{env::consts, fmt::Write as _, path::Path, process::ExitCode};

/// The schema of the document `setup` prints under `--json`.
const SCHEMA: &str = "maestro-cli/setup/1";

/// The schema of the document `setup` prints under `--json` before it
/// refuses or fails the search service's part: the graph's part alone.
const GRAPH_SCHEMA: &str = "maestro-cli/setup-graph/1";

/// What `setup` prints under `--json`.
#[derive(Debug, Serialize)]
struct SetupDocument<'a> {
    /// [`SCHEMA`].
    schema: &'static str,
    /// The version of Qdrant it installs.
    version: &'a str,
    /// The systemd user unit that runs it.
    service: &'static str,
    /// Where its binary is.
    binary: String,
    /// Where it keeps its collections.
    storage: String,
    /// Where it keeps its snapshots.
    snapshots: String,
    /// Where its unit is.
    unit: String,
    /// The address of its HTTP API.
    http: String,
    /// The address of its gRPC API.
    grpc: String,
    /// The embedded graph's part.
    graph: &'a GraphSetup,
    /// The steps the install lacks, which `--yes` takes: none when
    /// everything is in place.
    steps: Vec<&'static str>,
    /// Whether this run took them.
    changed: bool,
}

/// What `setup` prints under `--json` when the search service's part is
/// refused or fails: the graph's part, which came first.
#[derive(Debug, Serialize)]
struct GraphDocument<'a> {
    /// [`GRAPH_SCHEMA`].
    schema: &'static str,
    /// The embedded graph's part.
    graph: &'a GraphSetup,
}

/// What stands between this machine and the search service setup installs.
#[derive(Debug)]
pub(in crate::cli) enum Readiness {
    /// Setup installs nothing on this platform: Qdrant is set up by hand.
    ByHand,
    /// The steps setup would take; none when everything is in place.
    Steps(Vec<Step>),
}

/// Previews the graph's directory for the `graph.engine` the `--set` flags
/// `flags` resolve, then the install, or takes their steps when `yes` is
/// given, and prints both and what they lacked. The graph's part runs on
/// every platform; when the service's part is refused or fails, the graph's
/// is printed alone first.
///
/// # Errors
///
/// As [`service`], when its part is refused or fails. A graph refusal is
/// reported independently and does not prevent the service part from running.
pub(in crate::cli) fn run(
    output: Output,
    yes: bool,
    flags: &[String],
) -> Result<ExitCode, Failure> {
    let environment = Environment::current();
    let graph = Session::for_cli(flags)
        .and_then(|session| GraphEngine::from_session(&session))
        .and_then(|engine| graph_setup(&environment, engine, yes));
    let (graph, graph_refusal) = match graph {
        Ok(graph) => (graph, None),
        Err(failure) => {
            let detail = failure.to_string();
            (
                GraphSetup {
                    engine: "unknown",
                    directory: None,
                    action: "refused",
                    missing_guards: Vec::new(),
                    changed: false,
                    detail: Some(detail.clone()),
                },
                Some(detail),
            )
        }
    };
    if let Some(detail) = &graph_refusal {
        diagnose(detail);
    }
    match service(&environment, yes) {
        Ok(service) => {
            report(output, &service, &graph)?;
            Ok(if graph_refusal.is_some() {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            })
        }
        Err(failure) => {
            let document = GraphDocument {
                schema: GRAPH_SCHEMA,
                graph: &graph,
            };
            output.result(&document, &graph_text(&graph))?;
            Err(failure)
        }
    }
}

/// The search service's part of a run: where it lives, the release, the
/// steps it lacked, and whether they were taken.
struct Service<'a> {
    /// Where it lives.
    layout: Layout,
    /// The release it installs.
    release: Release<'a>,
    /// The steps it lacked.
    steps: Vec<Step>,
    /// Whether this run took them.
    changed: bool,
}

/// Previews the install, or takes its steps when `yes` is given.
///
/// # Errors
///
/// As [`run`], for the search service.
fn service(environment: &Environment, yes: bool) -> Result<Service<'static>, Failure> {
    let layout = layout(environment)?;
    let release = release_for(consts::OS, consts::ARCH).map_err(|unsupported| {
        Failure::refused(unsupported.manual_steps(&layout.storage, &layout.snapshots))
    })?;
    let tools = Tools::on_path();
    user_manager(&tools)?;
    let steps = survey(&layout, &release, &tools)?;
    let changed = yes && !steps.is_empty();
    if changed {
        apply(&steps, &layout, &release, &tools)?;
    }
    Ok(Service {
        layout,
        release,
        steps,
        changed,
    })
}

/// What stands between this machine and the search service, as the
/// environment places it, asking only, through `tools`: whether the service
/// is enabled and running.
///
/// # Errors
///
/// As [`survey`], and [`Failure::Failed`] when a directory cannot be
/// resolved.
pub(in crate::cli) fn readiness(
    environment: &Environment,
    tools: &Tools,
) -> Result<Readiness, Failure> {
    let layout = layout(environment)?;
    match release_for(consts::OS, consts::ARCH) {
        Ok(release) => Ok(Readiness::Steps(survey(&layout, &release, tools)?)),
        Err(_) => Ok(Readiness::ByHand),
    }
}

/// Where the service lives on this machine: under the kernel's data
/// directory, its unit under the configuration home the kernel's
/// configuration directory is in.
///
/// # Errors
///
/// [`Failure::Failed`] when either directory cannot be resolved.
fn layout(environment: &Environment) -> Result<Layout, Failure> {
    let data = paths::data_dir(environment).map_err(|error| Failure::failed_by(&error))?;
    let config = paths::config_dir(environment).map_err(|error| Failure::failed_by(&error))?;
    Ok(Layout::new(&data, config.parent().unwrap_or(&config)))
}

/// Prints the `graph`'s part, then the `service` its layout places and the
/// steps it lacked, taken when it changed.
fn report(output: Output, service: &Service<'_>, graph: &GraphSetup) -> Result<ExitCode, Failure> {
    let Service {
        layout,
        release,
        steps,
        changed,
    } = service;
    let changed = *changed;
    let (http, grpc) = (format!("{HOST}:{HTTP_PORT}"), format!("{HOST}:{GRPC_PORT}"));
    let document = SetupDocument {
        schema: SCHEMA,
        version: release.version,
        service: SERVICE,
        binary: shown(&layout.binary),
        storage: shown(&layout.storage),
        snapshots: shown(&layout.snapshots),
        unit: shown(&layout.unit),
        http: http.clone(),
        grpc: grpc.clone(),
        graph,
        steps: steps.iter().map(|step| name(*step)).collect(),
        changed,
    };
    let mut text = vec![
        graph_text(graph),
        format!(
            "Qdrant {}, as the user service {SERVICE}, bound to {HOST} with telemetry off:",
            release.version
        ),
        format!("  binary     {}", document.binary),
        format!("  storage    {}", document.storage),
        format!("  snapshots  {}", document.snapshots),
        format!("  unit       {}", document.unit),
        format!("  ports      {http} (HTTP), {grpc} (gRPC)"),
    ];
    if steps.is_empty() {
        text.push("Everything is in place: nothing to change.".to_owned());
    } else {
        text.push(if changed {
            "Done:".to_owned()
        } else {
            "To do, which `maestro setup --yes` does:".to_owned()
        });
        text.extend(
            steps
                .iter()
                .map(|step| format!("  {}", described(*step, release))),
        );
    }
    if changed {
        text.push("`maestro doctor` checks that it answers.".to_owned());
    }
    output.result(&document, &text.join("\n"))?;
    Ok(ExitCode::SUCCESS)
}

/// The `graph`'s part, for people.
pub(super) fn graph_text(graph: &GraphSetup) -> String {
    if let Some(detail) = &graph.detail {
        return format!("Graph: refused: {detail}");
    }
    let Some(directory) = &graph.directory else {
        return "Graph: off (graph.engine = none), nothing to do.".to_owned();
    };
    let state = match (graph.action, graph.changed) {
        ("create_directory", true) => "created",
        ("create_directory", false) => "to create, which `maestro setup --yes` does",
        ("secure_permissions", true) => "given mode 0700",
        ("secure_permissions", false) => "to give mode 0700, which `maestro setup --yes` does",
        _ => "in place",
    };
    let mut text = format!("Graph: the embedded engine's directory {directory}, {state}.");
    if !graph.missing_guards.is_empty() {
        let guards = graph.missing_guards.join(", ");
        let state = if graph.changed {
            "created"
        } else {
            "missing; run maestro setup --yes to create"
        };
        let _ = write!(text, " Permanent guards {guards}: {state}.");
    }
    text
}

/// `path` as a document shows it.
fn shown(path: &Path) -> String {
    path.display().to_string()
}

/// The name of `step` in a document.
fn name(step: Step) -> &'static str {
    match step {
        Step::Install => "install",
        Step::WriteUnit => "write_unit",
        Step::Reload => "reload",
        Step::Enable => "enable",
        Step::Restart => "restart",
    }
}

/// What `step` does, for people.
fn described(step: Step, release: &Release<'_>) -> String {
    match step {
        Step::Install => format!(
            "download {}, check it and the binary it holds against their pinned SHA-256, \
             and install the binary",
            release.archive
        ),
        Step::WriteUnit => "write the unit".to_owned(),
        Step::Reload => "have systemd's user manager read the unit again".to_owned(),
        Step::Enable => "enable the service, which starts it at login".to_owned(),
        Step::Restart => "start the service, or restart it on what changed".to_owned(),
    }
}
