//! `knowledge import`: the import of a collection's corpus manifests run as
//! a leased job in the foreground (plan D5, T016). The job's key comes from
//! its kind, its collection's scope and its frozen inputs, the collection
//! and the digests of its declaration and of each source's manifest, and it
//! holds the collection's import as its resource. Its ID is printed first;
//! each hundred lines of a manifest, and each manifest's last, are a step
//! journaled under the lease, which a heartbeat thread renews in between;
//! and the command exits with the job's outcome.
//!
//! The same command again finds the job of its key: it prints one that
//! succeeded, and follows one another process holds, trying its lease about
//! once a second, so that it takes the job over once that lease expired, as
//! it takes over at once one that expired before. An import of other inputs
//! that holds the collection's import, whose lease expired or that no process
//! took, is superseded: cancelled, then this import submitted again, once.
//! One a live lease holds refuses this import, naming its job.

use super::{
    collection::{self, Declared},
    failure::{Failure, chain},
    kernel::Kernel,
    lease::{Holder, TIMING, holder},
    output::Output,
    wait::{self, Follower, POLL},
};
use maestro_kernel::{
    artifact::Digest,
    binding::Bindings,
    job::{self, Job, JobState, Lease, NewJob},
    store::Database,
};
use maestro_knowledge::{
    collection::Declaration,
    import::{self, Report},
};
use serde_json::{Map, Value, json};
use std::{fs, ops::ControlFlow, process::ExitCode, thread, time::SystemTime};
use ulid::Ulid;

/// The kind of the job an import runs.
const KIND: &str = "knowledge.import";
/// The schema of the document `knowledge import` prints under `--json`.
const SCHEMA: &str = "maestro-cli/import/1";
/// How many looks at a job another process holds pass between two tries of
/// its lease: about a second, at one look every [`POLL`].
const LOOKS_PER_TRY: u32 = 10;

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
    let job = submit(&kernel.database, &new)?;
    output.job(job.id)?;
    let mut follower = Follower::new(kernel, output, job.id);
    loop {
        if let Some(lease) = take(&kernel.database, job.id)? {
            return work(kernel, output, &declared.declaration, &bindings, lease);
        }
        for _ in 0..LOOKS_PER_TRY {
            if let Some(ended) = follower.look()? {
                return wait::report(output, SCHEMA, &ended);
            }
            thread::sleep(POLL);
        }
    }
}

/// Submits `new` in `database`, or finds the job of its key. When another
/// job holds its resource, that job is superseded if it may be, and `new`
/// submitted again, once.
///
/// # Errors
///
/// [`Failure::Refused`], naming the job, when a live lease holds the
/// resource's job, or another job holds it again; [`Failure::Failed`] when
/// the kernel fails.
fn submit(database: &Database, new: &NewJob<'_>) -> Result<Job, Failure> {
    let mut submitted = database.submit_job(new, SystemTime::now());
    if let Err(job::Error::ResourceHeld { job: holding, .. }) = submitted {
        let free =
            supersede(database, holding, new.inputs).map_err(|error| Failure::failed_by(&error))?;
        if free {
            submitted = database.submit_job(new, SystemTime::now());
        }
    }
    submitted.map_err(|error| match error {
        job::Error::ResourceHeld { .. } => Failure::refused_by(&error),
        _ => Failure::failed_by(&error),
    })
}

/// Supersedes `holding`, the job that holds the resource an import of
/// `inputs` needs, unless a live lease holds it: its lease is taken for this
/// process, and it is cancelled with `{"superseded_by": {"inputs": inputs}}`.
/// Returns whether the resource is free now: after that, or when the job
/// ended meanwhile, and not while a live lease holds it.
///
/// # Errors
///
/// The kernel's failures.
pub(super) fn supersede(
    database: &Database,
    holding: Ulid,
    inputs: &Value,
) -> Result<bool, job::Error> {
    let lease = match database.take_job(holding, &holder(), SystemTime::now(), TIMING.term) {
        Ok(lease) => lease,
        Err(job::Error::Held { .. }) => return Ok(false),
        Err(job::Error::IllegalMove { .. }) => return Ok(true),
        Err(error) => return Err(error),
    };
    let outcome = json!({ "superseded_by": { "inputs": inputs } });
    database.complete_job(&lease, JobState::Cancelled, &outcome)?;
    Ok(true)
}

/// The lease of job `id` in `database`, taken for this process; none while
/// another process holds a live lease, or once the job ended.
///
/// # Errors
///
/// [`Failure::Failed`] when the kernel fails.
fn take(database: &Database, id: Ulid) -> Result<Option<Lease>, Failure> {
    match database.take_job(id, &holder(), SystemTime::now(), TIMING.term) {
        Ok(lease) => Ok(Some(lease)),
        // The follower's next look finds a job that ended.
        Err(job::Error::Held { .. } | job::Error::IllegalMove { .. }) => Ok(None),
        Err(error) => Err(Failure::failed_by(&error)),
    }
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

/// Runs the import of `declaration` under `lease`, then ends its job with
/// its outcome and prints it.
fn work(
    kernel: &Kernel,
    output: Output,
    declaration: &Declaration,
    bindings: &Bindings,
    lease: Lease,
) -> Result<ExitCode, Failure> {
    let ended = Holder::run(&kernel.database, lease, TIMING, |holder| {
        let mut lost = None;
        let imported = import::import_observed(
            &kernel.database,
            &kernel.scopes,
            declaration,
            bindings,
            &mut |report: &Report| step(holder, output, report, &mut lost),
        );
        ending(imported, lost)
    })
    .map_err(|error| Failure::failed_by(&error))?;
    wait::report(output, SCHEMA, &ended)
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
