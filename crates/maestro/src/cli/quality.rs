//! `knowledge quality`: the quality gate (T020) over a collection, run as a
//! leased job in the foreground, as [`foreground`] runs one. The job's key
//! comes from its kind, its collection's scope and its frozen inputs: the
//! collection, the digest of its declaration, that of the quality ledger the
//! declaration names beside it, none when there is no such file, and the
//! digest of the revisions the local principal reads, their IDs in record
//! order, each followed by a line feed. The same command again finds the job
//! of its key, so it decides nothing twice; once an import records new
//! revisions, their digest makes a new job, which decides them. The job holds
//! the collection's quality gate, `collection/<id>/quality`, as its resource,
//! a heartbeat thread renews its lease while the gate runs, and it ends
//! succeeded with the gate's report, or failed with why the gate stopped:
//! what it decided before stays decided.

use super::{
    collection::{self, Declared},
    failure::{Failure, chain},
    foreground,
    kernel::Kernel,
    output::Output,
};
use maestro_kernel::{
    artifact::Digest,
    job::{JobState, NewJob},
};
use maestro_knowledge::quality::{self, Ledger, LedgerError, Report};
use serde_json::{Value, json};
use std::{fmt::Display, fs, io, process::ExitCode, str};

/// The kind of the job the gate runs.
const KIND: &str = "knowledge.quality";
/// The schema of the document `knowledge quality` prints under `--json`.
const SCHEMA: &str = "maestro-cli/knowledge-quality/1";

/// Runs the quality gate over the collection `collection` as a job, or finds
/// the job of the same gate, and prints it as it ended.
///
/// # Errors
///
/// [`Failure::Refused`] when the collection was not added, its quality
/// ledger is not strict or cannot be read, or a live lease holds another job
/// on the collection's quality gate; [`Failure::Failed`] when the kernel
/// fails.
pub(super) fn run(kernel: &Kernel, output: Output, collection: &str) -> Result<ExitCode, Failure> {
    let declared = collection::declared(kernel, collection)?;
    let (ledger, ledger_digest) = ledger(&declared)?;
    let inputs = json!({
        "collection": collection,
        "declaration": declared.digest.as_str(),
        "ledger": ledger_digest,
        "revisions": revisions(kernel, collection)?,
    });
    let scope = collection::collection_scope(collection)?;
    let resource = format!("collection/{collection}/quality");
    let new = NewJob {
        kind: KIND,
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    // The gate journals no step: the heartbeats alone renew its lease, over
    // a run of a minute or more on a large collection.
    foreground::run(kernel, output, &new, SCHEMA, |_| {
        ending(quality::gate(
            &kernel.database,
            &kernel.scopes,
            collection,
            &ledger,
        ))
    })
}

/// The quality ledger `declared` names, relative to its directory, with the
/// digest of its bytes; an empty ledger, and no digest, when there is no such
/// file.
///
/// # Errors
///
/// [`Failure::Refused`], naming the ledger, when it cannot be read or is not
/// a strict `maestro-quality-ledger/1`.
fn ledger(declared: &Declared) -> Result<(Ledger, Option<String>), Failure> {
    let directory = declared.path.parent().unwrap_or(&declared.path);
    let path = declared.declaration.quality.ledger.under(directory);
    let refused = |reason: &dyn Display| {
        Failure::refused(format!(
            "the quality ledger {} is refused: {reason}",
            path.display()
        ))
    };
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok((Ledger::default(), None));
        }
        Err(error) => return Err(refused(&error)),
    };
    let text = str::from_utf8(&bytes).map_err(|error| refused(&error))?;
    let ledger = text
        .parse()
        .map_err(|error: LedgerError| refused(&chain(&error)))?;
    Ok((ledger, Some(Digest::of(&bytes).as_str().to_owned())))
}

/// The digest of the revisions of the collection `collection` the local
/// principal reads: their IDs in record order, each followed by a line feed.
///
/// # Errors
///
/// [`Failure::Failed`] when the kernel fails.
fn revisions(kernel: &Kernel, collection: &str) -> Result<String, Failure> {
    let revisions = kernel
        .database
        .revisions(&kernel.scopes, collection)
        .map_err(|error| Failure::failed_by(&error))?;
    let mut ids = String::new();
    for revision in &revisions {
        ids.push_str(&revision.id);
        ids.push('\n');
    }
    Ok(Digest::of(ids.as_bytes()).as_str().to_owned())
}

/// How the job ends: succeeded with the gate's report, or failed with why
/// the gate stopped.
fn ending(gated: Result<Report, quality::Error>) -> (JobState, Value) {
    match gated {
        Ok(report) => (JobState::Succeeded, json!(report)),
        Err(error) => (JobState::Failed, json!({ "error": chain(&error) })),
    }
}
