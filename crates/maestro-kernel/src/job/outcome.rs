//! Outcomes: a job ends succeeded, failed or cancelled, and stays so.

use super::{
    error::Error,
    events::{moved_into, record_on_stream},
    lease::{held, lease_data},
    record::{COLUMNS, Job, Lease, find, job_row},
    state::JobState,
};
use crate::store::Database;
use rusqlite::{Transaction, params};
use serde_json::{Value, json};
use ulid::Ulid;

impl Database {
    /// Ends the job of `lease` in `to`, succeeded, failed or cancelled, with
    /// `outcome`, the JSON its holder reports, and returns the job as it
    /// ended, without a lease. An outcome is final: the job never moves
    /// again, and after a failure or a cancellation its key starts a new
    /// attempt. The same write records [`SUCCEEDED`](super::SUCCEEDED),
    /// [`FAILED`](super::FAILED) or [`CANCELLED`](super::CANCELLED) with the
    /// outcome and the lease it ended under.
    ///
    /// # Errors
    ///
    /// [`Error::Lost`] when `lease` is no longer the job's, taken over or
    /// ended; [`Error::IllegalMove`] when `to` is not an outcome;
    /// [`Error::UnknownJob`]; and [`Error::Store`] when the database cannot
    /// record the end or its event. Nothing is then recorded.
    pub fn complete_job(&self, lease: &Lease, to: JobState, outcome: &Value) -> Result<Job, Error> {
        self.write(|transaction| {
            let job = held(transaction, lease)?;
            if !job.state.may_move_to(to) {
                return Err(Error::IllegalMove {
                    job: lease.job,
                    from: job.state,
                    to,
                });
            }
            end(transaction, &job, to, outcome)
        })
    }

    /// Cancels job `id` while it is queued, with `outcome`, the JSON that
    /// tells why, and returns it as it ended; the same write records
    /// [`CANCELLED`](super::CANCELLED) with the outcome. A running job is
    /// cancelled by the holder of its lease, through
    /// [`Database::complete_job`], even once that lease has expired: whoever
    /// takes it over then cancels it.
    ///
    /// # Errors
    ///
    /// [`Error::Held`], naming its holder and its lease's expiry, when the
    /// job runs; [`Error::IllegalMove`] when it has ended;
    /// [`Error::UnknownJob`]; and [`Error::Store`] when the database cannot
    /// record the end or its event. Nothing is then recorded.
    pub fn cancel_job(&self, id: Ulid, outcome: &Value) -> Result<Job, Error> {
        self.write(|transaction| {
            let job = find(transaction, None, id)?.ok_or(Error::UnknownJob(id))?;
            if let Some(current) = &job.lease {
                return Err(Error::Held {
                    job: id,
                    holder: current.holder.clone(),
                    expires: current.expires.clone(),
                });
            }
            if !job.state.may_move_to(JobState::Cancelled) {
                return Err(Error::IllegalMove {
                    job: id,
                    from: job.state,
                    to: JobState::Cancelled,
                });
            }
            end(transaction, &job, JobState::Cancelled, outcome)
        })
    }
}

/// Ends `job` in `to` with `outcome` inside `transaction`: its lease goes, its
/// outcome is recorded, and so is the event of its end, with the outcome and
/// the lease it ended under, if any. Returns the job as it ended.
fn end(
    transaction: &Transaction<'_>,
    job: &Job,
    to: JobState,
    outcome: &Value,
) -> Result<Job, Error> {
    let ended = transaction.query_row(
        &format!(
            "UPDATE jobs
             SET state = ?2, outcome_json = ?3, lease_holder = NULL, lease_heartbeat = NULL,
               lease_expires = NULL
             WHERE id = ?1
             RETURNING {COLUMNS}"
        ),
        params![job.id.to_string(), to.as_str(), outcome.to_string()],
        job_row,
    )?;
    let data = match &job.lease {
        Some(lease) => json!({ "outcome": outcome, "lease": lease_data(lease) }),
        None => json!({ "outcome": outcome }),
    };
    record_on_stream(transaction, ended.id, &ended.scope, moved_into(to), &data)?;
    Ok(ended)
}
