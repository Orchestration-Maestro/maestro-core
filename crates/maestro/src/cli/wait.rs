//! `job wait <id>`: a job's stream followed until the job ends, each event
//! printed for people as it comes, then the job as it ended; the command
//! exits 0 when it succeeded and 1 when it failed or was cancelled. A job
//! the local principal cannot read is unknown to it.

use super::output::Output;
use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{
    job::{self, Job, JobState},
    journal::Filter,
    json::canonical,
};
use serde::Serialize;
use serde_json::Value;
use std::{process::ExitCode, thread, time::Duration};
use ulid::Ulid;

/// How `job wait` prints a job as it ended: `maestro-cli/job-wait/1`, or
/// [`line()`] for people.
const PRINTING: Printing = Printing {
    schema: "maestro-cli/job-wait/1",
    text: line,
};
/// How long a follower waits before it reads a running job's stream again.
pub(super) const POLL: Duration = Duration::from_millis(100);

/// A job as it ended, as `job wait` and `knowledge import` print it under
/// `--json`.
#[derive(Debug, Serialize)]
struct JobDocument<'a> {
    /// The command's schema.
    schema: &'static str,
    /// The job's ID.
    job: String,
    /// Its kind, such as `knowledge.import`.
    kind: &'a str,
    /// Its attempt among the jobs of its key.
    attempt: u64,
    /// The state it ended in.
    state: String,
    /// The JSON it ended with.
    #[serde(serialize_with = "maestro_kernel::json::serialize_canonical")]
    outcome: &'a Value,
}

/// Follows job `id` to its end, then prints it.
///
/// # Errors
///
/// As [`Follower::look`].
pub(super) fn run(kernel: &Kernel, output: Output, id: Ulid) -> Result<ExitCode, Failure> {
    let mut follower = Follower::new(kernel, output, id);
    loop {
        if let Some(ended) = follower.look()? {
            return report(output, PRINTING, &ended);
        }
        thread::sleep(POLL);
    }
}

/// A follower of a job's stream: it prints each event for people as it
/// reads it, and knows how far it has read.
#[derive(Debug)]
pub(super) struct Follower<'a> {
    /// The kernel the job is recorded in.
    kernel: &'a Kernel,
    /// How the events are printed.
    output: Output,
    /// The job.
    id: Ulid,
    /// The job's stream.
    stream: String,
    /// The sequence of the last event printed; 0 before the first.
    after: u64,
}

impl<'a> Follower<'a> {
    /// A follower of job `id`, from its first event.
    pub(super) fn new(kernel: &'a Kernel, output: Output, id: Ulid) -> Self {
        Self {
            kernel,
            output,
            id,
            stream: job::stream(id),
            after: 0,
        }
    }

    /// Prints the events of the job recorded since the last look, and
    /// returns the job once it ended.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] when the local principal reads no such job, and
    /// [`Failure::Failed`] when the kernel fails.
    pub(super) fn look(&mut self) -> Result<Option<Job>, Failure> {
        let id = self.id;
        // The job before its stream: the write that ends a job records its
        // last event, so the stream read after a job that ended holds it.
        let job = self
            .kernel
            .database
            .job(&self.kernel.scopes, id)
            .map_err(|error| Failure::failed_by(&error))?
            .ok_or_else(|| {
                Failure::refused(format!(
                    "no job {id} is recorded that the local principal reads"
                ))
            })?;
        let filter = Filter {
            stream: &self.stream,
            after: self.after,
            r#type: None,
        };
        let events = self
            .kernel
            .database
            .events(&self.kernel.scopes, &filter)
            .map_err(|error| Failure::failed_by(&error))?;
        for event in events {
            self.output
                .text(&progress_line(event.sequence, &event.r#type, event.data))?;
            self.after = event.sequence;
        }
        let ended = matches!(
            job.state,
            JobState::Succeeded | JobState::Failed | JobState::Cancelled
        );
        Ok(ended.then_some(job))
    }
}

/// How a command prints a job as it ended: under `--json`, the document of
/// `schema`; for people, the text `text` gives.
#[derive(Debug, Clone, Copy)]
pub(super) struct Printing {
    /// The schema of the document, such as `maestro-cli/import/1`.
    pub(super) schema: &'static str,
    /// The job as text for people.
    pub(super) text: fn(&Job) -> String,
}

/// `job`, which ended, for people: its state, then its outcome's JSON.
pub(super) fn line(job: &Job) -> String {
    let outcome = job.outcome.as_ref().unwrap_or(&Value::Null);
    let outcome = canonical(outcome.clone());
    format!("{} {outcome}", job.state)
}

/// Prints `job`, which ended, as `printing` says, and returns the exit code
/// of its outcome: 0 when it succeeded, 1 otherwise.
///
/// # Errors
///
/// [`Failure::Failed`] when stdout cannot be written to.
pub(super) fn report(output: Output, printing: Printing, job: &Job) -> Result<ExitCode, Failure> {
    let outcome = job.outcome.as_ref().unwrap_or(&Value::Null);
    let document = JobDocument {
        schema: printing.schema,
        job: job.id.to_string(),
        kind: &job.kind,
        attempt: job.attempt,
        state: job.state.to_string(),
        outcome,
    };
    output.result(&document, &(printing.text)(job))?;
    Ok(if job.state == JobState::Succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// One event's text, omitting the final outcome printed by the ending report.
fn progress_line(sequence: u64, kind: &str, mut data: Value) -> String {
    data = canonical(data);
    if matches!(kind, job::SUCCEEDED | job::FAILED | job::CANCELLED) {
        drop(data.as_object_mut().map(|data| data.remove("outcome")));
    }
    format!("{sequence} {kind} {data}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_kernel::{artifact::Digest, scope::WORKSPACE};

    #[test]
    fn progress_and_outcome_formatting_keep_pre_cedar_bytes() {
        let data = serde_json::from_str(r#"{"z":{"z":2,"a":1},"a":0}"#).unwrap();
        assert_eq!(
            progress_line(3, job::PROGRESSED, data),
            r#"3 maestro.job.progressed.v1 {"a":0,"z":{"a":1,"z":2}}"#
        );
        let outcome = serde_json::from_str(r#"{"z":{"z":2,"a":1},"a":0}"#).unwrap();
        let job = Job {
            id: Ulid::from(0_u128),
            kind: "synthetic".into(),
            idempotency_key: Digest::of(b"synthetic"),
            attempt: 1,
            scope: WORKSPACE.parse().unwrap(),
            resource: None,
            state: JobState::Succeeded,
            lease: None,
            outcome: Some(outcome),
        };
        assert_eq!(line(&job), r#"succeeded {"a":0,"z":{"a":1,"z":2}}"#);
        let document = JobDocument {
            schema: PRINTING.schema,
            job: job.id.to_string(),
            kind: &job.kind,
            attempt: 1,
            state: job.state.to_string(),
            outcome: job.outcome.as_ref().unwrap(),
        };
        assert_eq!(
            serde_json::to_string(&document).unwrap(),
            concat!(
                "{\"schema\":\"maestro-cli/job-wait/1\",",
                "\"job\":\"00000000000000000000000000\",\"kind\":\"synthetic\",",
                "\"attempt\":1,\"state\":\"succeeded\",",
                "\"outcome\":{\"a\":0,\"z\":{\"a\":1,\"z\":2}}}",
            )
        );
    }
}
