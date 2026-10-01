//! `knowledge verify`: checks a published generation as a leased job.

use super::{
    collection, foreground, health,
    output::Output,
    wait::{self, Printing},
};
use crate::{
    failure::{Failure, chain},
    kernel::Kernel,
};
use maestro_kernel::job::{JobState, NewJob};
use maestro_knowledge::{
    index::Qdrant,
    publish::{Verification, verify_generation},
};
use serde_json::{Value, json};
use std::{env, process::ExitCode};
use tokio::runtime::Builder;
use ulid::Ulid;

/// The kind of the job verification runs.
const KIND: &str = "knowledge.verify";
/// The schema of `knowledge verify`'s JSON document.
const SCHEMA: &str = "maestro-cli/knowledge-verify/1";
/// How `knowledge verify` prints its ended job.
const PRINTING: Printing = Printing {
    schema: SCHEMA,
    text: wait::line,
};

/// Verifies the published generation of `collection`, or refuses an
/// unknown collection or one without a published generation.
pub(super) fn run(
    kernel: &Kernel,
    output: Output,
    collection_id: &str,
) -> Result<ExitCode, Failure> {
    let collection = kernel
        .database
        .collection(&kernel.scopes, collection_id)
        .map_err(|error| Failure::failed_by(&error))?
        .ok_or_else(|| {
            Failure::refused(format!(
                "no collection {collection_id} is recorded that the local principal reads"
            ))
        })?;
    let generation = kernel
        .database
        .published_generation(&kernel.scopes, collection_id)
        .map_err(|error| Failure::failed_by(&error))?
        .ok_or_else(|| {
            Failure::refused(format!(
                "collection {collection_id} has no published generation"
            ))
        })?;
    let url =
        env::var(health::QDRANT_VARIABLE).unwrap_or_else(|_| health::DEFAULT_QDRANT.to_owned());
    let qdrant = Qdrant::new(&url).map_err(|error| Failure::refused_by(&error))?;
    let inputs = json!({
        "again": Ulid::generate().to_string(),
        "collection": collection.id,
        "generation": generation.id,
    });
    let scope = collection::collection_scope(collection_id)?;
    let resource = format!("collection/{collection_id}/verify");
    let new = NewJob {
        kind: KIND,
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    foreground::run(
        kernel,
        output,
        &new,
        |_| {
            let runtime = match Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    return (
                        JobState::Failed,
                        json!({"error": format!("cannot start verification runtime: {error}")}),
                    );
                }
            };
            match runtime.block_on(verify_generation(
                &kernel.database,
                &kernel.scopes,
                &qdrant,
                generation.id,
            )) {
                Ok(report) => outcome(&report),
                Err(error) => (JobState::Failed, json!({"error": chain(&error)})),
            }
        },
        PRINTING,
    )
}

/// The job's state and outcome for a completed verification report.
pub(super) fn outcome(report: &Verification) -> (JobState, Value) {
    let state = if report.findings.is_empty() {
        JobState::Succeeded
    } else {
        JobState::Failed
    };
    (state, json!(report))
}
