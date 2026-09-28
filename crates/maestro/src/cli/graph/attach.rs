//! Attachment is separate leased work; completed extraction is never repeated.

use super::{failure::claim_failure, job};
use crate::{
    cli::{foreground, lease::TIMING, output::Output},
    failure::Failure,
    kernel::Kernel,
};
use maestro_kernel::{job::NewJob, scope::collection_path};
use serde::Serialize;
use serde_json::json;
use std::process::ExitCode;
use ulid::Ulid;

/// Attach an existing frozen build under a new job's fence, idempotently.
pub(super) fn attach(
    kernel: &Kernel,
    output: Output,
    build: Ulid,
    generation: i64,
) -> Result<Ulid, Failure> {
    let record = kernel
        .database
        .graph_build(&kernel.scopes, build)
        .map_err(|error| claim_failure(&error))?
        .ok_or_else(|| Failure::refused("unknown graph build"))?;
    if record.claim_set_id.is_none() {
        return Err(Failure::refused("graph build has no finished claim set"));
    }
    let scope = collection_path(&record.plan.collection_id)
        .parse()
        .map_err(|error| Failure::refused_by(&error))?;
    let inputs = json!({"build": build.to_string(), "generation": generation});
    let resource = format!("graph-attach:{generation}");
    let new = NewJob {
        kind: "knowledge.graph.attach",
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    let ended = foreground::run_to_end(kernel, output, &new, TIMING, |holder| {
        job::outcome(
            holder
                .fenced(|lease, _| {
                    kernel
                        .database
                        .attach_claim_set(&kernel.scopes, generation, build, lease)
                })
                .map(drop)
                .map_err(|error| claim_failure(&error)),
        )
    })?;
    job::check(&ended)?;
    Ok(ended.id)
}

/// Standalone attachment command, sharing exactly the build command's attach path.
pub(in crate::cli) fn run(
    kernel: &Kernel,
    output: Output,
    build: Ulid,
    generation: i64,
) -> Result<ExitCode, Failure> {
    let job = attach(kernel, output, build, generation)?;
    let document = Attached {
        schema: "maestro-cli/knowledge-graph-attach/1",
        job: job.to_string(),
        build: build.to_string(),
        generation,
    };
    output.result(&document, "graph claim set attached")?;
    Ok(ExitCode::SUCCESS)
}

/// The attachment document keeps its schema first, like other CLI documents.
#[derive(Serialize)]
struct Attached {
    /// Versioned wire contract.
    schema: &'static str,
    /// Attachment job identity.
    job: String,
    /// Frozen build job identity.
    build: String,
    /// Target generation identity.
    generation: i64,
}
