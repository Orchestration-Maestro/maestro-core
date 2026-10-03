//! Cleanup only a selected authorized receipt file; the knowledge layer owns deletion policy.
use crate::{
    cli::{
        foreground,
        lease::TIMING,
        output::{Output, diagnose},
    },
    failure::Failure,
    kernel::Kernel,
};
use maestro_kernel::{
    job::{JobState, NewJob},
    paths::{self, Environment},
    scope::{LOCAL, collection_path},
};
use maestro_knowledge::graph::projection::cleanup::{Cleanup, CleanupError, CleanupOutcome};
use serde::Serialize;
use serde_json::json;
use std::{cell::Cell, process::ExitCode};

/// Preview by default; explicit confirmation applies under the same held root guard.
pub(in crate::cli) fn run(
    kernel: &Kernel,
    output: Output,
    generation: i64,
    yes: bool,
) -> Result<ExitCode, Failure> {
    let data =
        paths::data_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))?;
    let cleanup = match Cleanup::prepare(
        &kernel.database,
        LOCAL,
        &data.join("graph"),
        generation,
        yes,
    ) {
        Ok(cleanup) => cleanup,
        Err(error) => return refuse(output, generation, error),
    };
    if !yes {
        let reason = if cleanup.present() {
            "candidate"
        } else {
            "already_missing"
        };
        let document = Document::candidate(&cleanup, "preview", reason, cleanup.present());
        let presence = if cleanup.present() {
            "present"
        } else {
            "already missing"
        };
        output.result(
            &document,
            &format!(
                "graph cleanup preview: generation {generation}, file {}, \
             {presence}; repeat with --yes to apply",
                cleanup.receipt().file_name
            ),
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    apply(kernel, output, &cleanup)
}

/// Run the scoped existing job protocol while the candidate's exclusive guard stays held.
fn apply(kernel: &Kernel, output: Output, cleanup: &Cleanup) -> Result<ExitCode, Failure> {
    let generation = cleanup.receipt().generation_id;
    let scope = collection_path(&cleanup.receipt().collection_id)
        .parse()
        .map_err(|error| Failure::refused_by(&error))?;
    let inputs = json!({"generation": generation});
    let resource = format!("graph-cleanup:{generation}");
    let new = NewJob {
        kind: "knowledge.graph.cleanup",
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    let result = Cell::new(None);
    let ended = foreground::run_to_end(kernel, output, &new, TIMING, |holder| {
        let applied = holder.fenced(|lease, timing| {
            // Config revocations apply even after the candidate and lease were selected.
            kernel
                .database
                .refresh_config(&kernel.config_dir)
                .map_err(|_| CleanupError::AuthorityUnavailable)?;
            cleanup.apply(&kernel.database, LOCAL, lease, timing)
        });
        result.set(Some(applied));
        match applied {
            Ok(outcome) => (JobState::Succeeded, json!({"reason": outcome.reason()})),
            Err(error) => (JobState::Failed, json!({"reason": error.reason()})),
        }
    });
    let Ok(ended) = ended else {
        return refuse(output, generation, CleanupError::LeaseInvalid);
    };
    let applied = result
        .get()
        .unwrap_or_else(|| replay_outcome(ended.state, cleanup.present()));
    let outcome = match applied {
        Ok(outcome) => outcome,
        Err(error) => return refuse(output, generation, error),
    };
    let mut document = Document::candidate(cleanup, "applied", outcome.reason(), false);
    document.job = Some(ended.id.to_string());
    output.result(
        &document,
        &format!(
            "graph cleanup applied: generation {generation}, file {}, {}",
            cleanup.receipt().file_name,
            outcome.reason()
        ),
    )?;
    Ok(ExitCode::SUCCESS)
}

/// A replayed successful job authorizes only idempotent absence, never a fresh deletion.
fn replay_outcome(state: JobState, present: bool) -> Result<CleanupOutcome, CleanupError> {
    if state == JobState::Succeeded && !present {
        Ok(CleanupOutcome::AlreadyMissing)
    } else {
        Err(CleanupError::LeaseInvalid)
    }
}

/// Report the same fixed refusal in JSON and text; no candidate is disclosed.
fn refuse(output: Output, generation: i64, error: CleanupError) -> Result<ExitCode, Failure> {
    let document = Document {
        schema: "maestro-cli/knowledge-graph-cleanup/1",
        action: "refused",
        reason: error.reason(),
        generation,
        collection: None,
        file_name: None,
        present: None,
        job: None,
    };
    output.refusal(&document, &error.to_string())?;
    if output.is_json() {
        diagnose(&error.to_string());
    }
    let code = if error == CleanupError::AuthorityUnavailable {
        1
    } else {
        2
    };
    Ok(ExitCode::from(code))
}

/// Closed versioned command document, schema first; refusals contain no hidden target identity.
#[derive(Serialize)]
struct Document<'a> {
    /// Wire format version.
    schema: &'static str,
    /// Preview, applied or refused.
    action: &'static str,
    /// Fixed policy/outcome reason.
    reason: &'static str,
    /// Explicit generation selection supplied by the caller.
    generation: i64,
    /// Authorized collection only.
    #[serde(skip_serializing_if = "Option::is_none")]
    collection: Option<&'a str>,
    /// Exact immutable receipt basename only.
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<&'a str>,
    /// Preview or post-apply absence state.
    #[serde(skip_serializing_if = "Option::is_none")]
    present: Option<bool>,
    /// Apply's durable cleanup job; preview creates none.
    #[serde(skip_serializing_if = "Option::is_none")]
    job: Option<String>,
}

impl<'a> Document<'a> {
    /// Authorized candidate fields come from the guarded knowledge operation, never guessed names.
    fn candidate(
        cleanup: &'a Cleanup,
        action: &'static str,
        reason: &'static str,
        present: bool,
    ) -> Self {
        Self {
            schema: "maestro-cli/knowledge-graph-cleanup/1",
            action,
            reason,
            generation: cleanup.receipt().generation_id,
            collection: Some(&cleanup.receipt().collection_id),
            file_name: Some(&cleanup.receipt().file_name),
            present: Some(present),
            job: None,
        }
    }
}

#[cfg(test)]
mod replay_tests {
    use super::{CleanupError, CleanupOutcome, JobState, replay_outcome};

    #[test]
    fn graph_cleanup_replay_requires_success_and_absence() {
        assert_eq!(
            replay_outcome(JobState::Succeeded, false),
            Ok(CleanupOutcome::AlreadyMissing)
        );
        for (state, present) in [
            (JobState::Succeeded, true),
            (JobState::Failed, false),
            (JobState::Failed, true),
        ] {
            assert_eq!(
                replay_outcome(state, present),
                Err(CleanupError::LeaseInvalid)
            );
        }
    }
}
