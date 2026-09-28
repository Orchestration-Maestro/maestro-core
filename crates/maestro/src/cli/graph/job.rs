//! Foreground graph builds: extraction outside the fence lock, atomic receipts inside it.

use super::failure::claim_failure;
use crate::{
    cli::{
        foreground,
        lease::{Holder, TIMING},
        output::Output,
    },
    failure::Failure,
    kernel::Kernel,
};
use maestro_kernel::{
    document::Revision,
    facts::{BuildPlan, BuildRecord, ClaimSetRecord},
    job::{Job, JobState, NewJob},
    scope::collection_path,
};
use maestro_knowledge::graph::{build, rules::Extractor};
use serde_json::{Value, json};
use ulid::Ulid;

/// Frozen work supplied to the foreground holder.
pub(super) struct Work<'a> {
    /// The full immutable input contract.
    pub(super) plan: &'a BuildPlan,
    /// Revisions in exactly the plan order.
    pub(super) revisions: &'a [Revision],
    /// Replaceable extraction implementation.
    pub(super) extractor: &'a dyn Extractor,
}

/// The authority-backed result, including a job identity for later attachment.
pub(super) struct Built {
    /// Build job.
    pub(super) job: Ulid,
    /// Receipts and retained rejections.
    pub(super) record: BuildRecord,
    /// No set when the completed extraction accepted nothing.
    pub(super) set: Option<ClaimSetRecord>,
}

/// Submit or resume exactly this plan, using the existing foreground lease runner.
pub(super) fn run(kernel: &Kernel, output: Output, work: &Work<'_>) -> Result<Built, Failure> {
    let inputs = build::inputs(work.plan);
    let scope = collection_path(&work.plan.collection_id)
        .parse()
        .map_err(|error| Failure::refused_by(&error))?;
    let resource = format!("graph-build:{}", work.plan.collection_id);
    let new = NewJob {
        kind: "knowledge.graph.build",
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    let ended = foreground::run_to_end(kernel, output, &new, TIMING, |holder| {
        outcome(work.run(kernel, holder))
    })?;
    check(&ended)?;
    let record = kernel
        .database
        .graph_build(&kernel.scopes, ended.id)
        .map_err(|error| claim_failure(&error))?
        .ok_or_else(|| Failure::failed("completed graph build has no receipts"))?;
    let set = record
        .claim_set_id
        .as_ref()
        .map(|id| kernel.database.claim_set(&kernel.scopes, id))
        .transpose()
        .map_err(|error| claim_failure(&error))?
        .flatten();
    Ok(Built {
        job: ended.id,
        record,
        set,
    })
}

impl Work<'_> {
    /// Resume at the durable receipt boundary, never repeating committed extraction.
    fn run(&self, kernel: &Kernel, holder: &Holder<'_>) -> Result<(), Failure> {
        let current = holder
            .fenced(|lease, _| {
                kernel
                    .database
                    .begin_graph_build(&kernel.scopes, lease, self.plan)
            })
            .map_err(|error| claim_failure(&error))?;
        let mut claims: usize = current.batches.iter().map(|batch| batch.claims.len()).sum();
        for (ordinal, revision) in self
            .revisions
            .iter()
            .enumerate()
            .skip(current.batches.len())
        {
            let batch = build::extract(&kernel.database, self.extractor, revision, ordinal)
                .map_err(|error| Failure::failed_by(&error))?;
            claims = claims.saturating_add(batch.claims.len());
            if claims > self.plan.budget.max_claims {
                return Err(Failure::refused(format!(
                    "max-claims bound {} exceeded (needed {claims}); raise --max-claims",
                    self.plan.budget.max_claims
                )));
            }
            holder
                .fenced(|lease, timing| {
                    kernel
                        .database
                        .record_graph_batch(&kernel.scopes, lease, timing, &batch)
                })
                .map_err(|error| claim_failure(&error))?;
        }
        if claims != 0 {
            holder
                .fenced(|lease, timing| {
                    kernel
                        .database
                        .finish_graph_build(&kernel.scopes, lease, timing)
                })
                .map_err(|error| claim_failure(&error))?;
        }
        Ok(())
    }
}

/// Preserve failure classification in the durable job outcome for followers.
pub(super) fn outcome(result: Result<(), Failure>) -> (JobState, Value) {
    match result {
        Ok(()) => (JobState::Succeeded, json!({})),
        Err(error) => (
            JobState::Failed,
            json!({"refused": matches!(error, Failure::Refused(_)), "error": error.to_string()}),
        ),
    }
}

/// A follower reports the same terminal refusal/failure as the worker.
pub(super) fn check(job: &Job) -> Result<(), Failure> {
    if job.state == JobState::Succeeded {
        return Ok(());
    }
    let outcome = job.outcome.as_ref();
    let message = outcome
        .and_then(|value| value["error"].as_str())
        .unwrap_or("graph job did not succeed");
    if outcome.is_some_and(|value| value["refused"] == true) {
        Err(Failure::refused(message))
    } else {
        Err(Failure::failed(message))
    }
}
