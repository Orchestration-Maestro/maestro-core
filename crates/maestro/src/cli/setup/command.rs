//! `maestro setup`: the search service's install, previewed, then done with
//! `--yes`. It opens no kernel: it only reads, then writes the service's own
//! files and asks systemd's user manager to run it.

use super::{
    release::{GRPC_PORT, HOST, HTTP_PORT, Release, SERVICE, release_for},
    service::{Layout, Step, apply, survey},
    tools::Tools,
};
use crate::cli::{failure::Failure, output::Output};
use maestro_kernel::paths::{self, Environment};
use serde::Serialize;
use std::{env::consts, path::Path, process::ExitCode};

/// The schema of the document `setup` prints under `--json`.
const SCHEMA: &str = "maestro-cli/setup/1";

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
    /// The steps the install lacks, which `--yes` takes: none when
    /// everything is in place.
    steps: Vec<&'static str>,
    /// Whether this run took them.
    changed: bool,
}

/// What stands between this machine and the search service setup installs.
#[derive(Debug)]
pub(in crate::cli) enum Readiness {
    /// Setup installs nothing on this platform: Qdrant is set up by hand.
    ByHand,
    /// The steps setup would take; none when everything is in place.
    Steps(Vec<Step>),
}

/// Previews the install, or takes its steps when `yes` is given, and prints
/// the service and what it lacked.
///
/// # Errors
///
/// [`Failure::Refused`] on a platform setup does not install on, with the
/// manual steps, or for a path no unit can hold, and [`Failure::Failed`]
/// when a directory cannot be resolved or a step fails.
pub(in crate::cli) fn run(output: Output, yes: bool) -> Result<ExitCode, Failure> {
    let environment = Environment::current();
    let layout = layout(&environment)?;
    let release = release_for(consts::OS, consts::ARCH).map_err(|unsupported| {
        Failure::refused(unsupported.manual_steps(&layout.storage, &layout.snapshots))
    })?;
    let tools = Tools::on_path();
    let steps = survey(&layout, &release, &tools)?;
    let changed = yes && !steps.is_empty();
    if changed {
        apply(&steps, &layout, &release, &tools)?;
    }
    report(output, &layout, &release, &steps, changed)
}

/// What stands between this machine and the search service, as the
/// environment places it, asking only.
///
/// # Errors
///
/// As [`survey`], and [`Failure::Failed`] when a directory cannot be
/// resolved.
pub(in crate::cli) fn readiness(environment: &Environment) -> Result<Readiness, Failure> {
    let layout = layout(environment)?;
    match release_for(consts::OS, consts::ARCH) {
        Ok(release) => Ok(Readiness::Steps(survey(
            &layout,
            &release,
            &Tools::on_path(),
        )?)),
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

/// Prints the service `layout` places and the `steps` it lacked, taken when
/// `changed`.
fn report(
    output: Output,
    layout: &Layout,
    release: &Release<'_>,
    steps: &[Step],
    changed: bool,
) -> Result<ExitCode, Failure> {
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
        steps: steps.iter().map(|step| name(*step)).collect(),
        changed,
    };
    let mut text = vec![
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
