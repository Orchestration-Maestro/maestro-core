//! `knowledge import`: the import of a collection's corpus manifests run as
//! a leased job in the foreground (plan D5, T016), as [`foreground`] runs
//! one. The job's key comes from its kind, its collection's scope and its
//! frozen inputs, the collection and the digests of its declaration and of
//! each source's manifest, and it holds the collection's import as its
//! resource. Its ID is printed first; each hundred lines of a manifest, and
//! each manifest's last, are a step journaled under the lease, which a
//! heartbeat thread renews in between; and the command exits with the job's
//! outcome. The same command again finds the job of its key, and an import
//! of other inputs that holds the collection's import is superseded or
//! refuses this one, as [`foreground`] says.

use super::{
    collection::{self, Declared},
    failure::{Failure, chain},
    foreground,
    kernel::Kernel,
    lease::Holder,
    output::Output,
};
use maestro_kernel::{
    artifact::Digest,
    binding::Bindings,
    job::{self, JobState, NewJob},
};
use maestro_knowledge::import::{self, Report};
use serde_json::{Map, Value, json};
use std::{fs, ops::ControlFlow, process::ExitCode};

/// The kind of the job an import runs.
const KIND: &str = "knowledge.import";
/// The schema of the document `knowledge import` prints under `--json`.
const SCHEMA: &str = "maestro-cli/import/1";

/// Imports the collection `collection` as a job, or finds the job of the
/// same import, and prints it as it ended.
///
/// # Errors
///
/// [`Failure::Refused`] when the collection was not added, a manifest's
/// binding is not bound or its manifest cannot be read, or a live lease holds
/// another job on the collection's import; [`Failure::Failed`] when the
/// kernel fails.
pub(super) fn run(kernel: &Kernel, output: Output, collection: &str) -> Result<ExitCode, Failure> {
    let declared = collection::declared(kernel, collection)?;
    let bindings =
        Bindings::load(&kernel.config_dir).map_err(|error| Failure::refused_by(&error))?;
    let inputs = inputs(&declared, &bindings)?;
    let scope = collection::collection_scope(collection)?;
    let resource = format!("collection/{collection}/import");
    let new = NewJob {
        kind: KIND,
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    foreground::run(kernel, output, &new, SCHEMA, |holder| {
        let mut lost = None;
        let imported = import::import_observed(
            &kernel.database,
            &kernel.scopes,
            &declared.declaration,
            &bindings,
            &mut |report: &Report| step(holder, output, report, &mut lost),
        );
        ending(imported, lost)
    })
}

/// The frozen inputs of the import of `declared`: its collection, the digest
/// of its declaration, and the digest of each source's manifest, found
/// through `bindings`, by source.
fn inputs(declared: &Declared, bindings: &Bindings) -> Result<Value, Failure> {
    let manifests = declared
        .declaration
        .manifest_paths(bindings)
        .map_err(|error| Failure::refused_by(&error))?;
    let mut digests = Map::new();
    for (source, path) in manifests {
        let bytes = fs::read(&path).map_err(|error| {
            Failure::refused(format!(
                "the manifest of the source `{}` cannot be read at {}: {error}",
                source.id,
                path.display()
            ))
        })?;
        digests.insert(source.id.clone(), Value::from(Digest::of(&bytes).as_str()));
    }
    Ok(json!({
        "collection": declared.declaration.id,
        "declaration": declared.digest.as_str(),
        "manifests": digests,
    }))
}

/// Journals the counts of `report` as the job's next step, and prints them
/// for people. When the step cannot be journaled, the lease taken over among
/// the reasons, it keeps why in `stopped` and breaks, which stops the import.
pub(super) fn step(
    holder: &Holder<'_>,
    output: Output,
    report: &Report,
    stopped: &mut Option<job::Error>,
) -> ControlFlow<()> {
    let data = json!({
        "imported": report.imported,
        "unchanged": report.unchanged,
        "held": report.held,
        "refused": report.refused,
    });
    match holder.step(&data) {
        Ok(()) => {
            // A step is journaled whether or not stdout still takes it.
            drop(output.text(&format!("step {data}")));
            ControlFlow::Continue(())
        }
        Err(error) => {
            *stopped = Some(error);
            ControlFlow::Break(())
        }
    }
}

/// How the job ends: succeeded with the report of an import that ended, or
/// failed with why the import, or the journaling of its steps, stopped.
pub(super) fn ending(
    imported: Result<Report, import::Error>,
    stopped: Option<job::Error>,
) -> (JobState, Value) {
    match (imported, stopped) {
        (_, Some(error)) => (JobState::Failed, json!({ "error": chain(&error) })),
        (Ok(report), None) => (JobState::Succeeded, json!(report)),
        (Err(error), None) => (JobState::Failed, json!({ "error": chain(&error) })),
    }
}
