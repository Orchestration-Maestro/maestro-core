//! Outcomes: a job ends succeeded, failed or cancelled, and stays so.

use super::{
    error::Error,
    lease::held,
    record::{COLUMNS, Job, Lease, find, job_row},
    state::JobState,
};
use crate::store::Database;
use rusqlite::{Transaction, params};
use serde_json::Value;
use ulid::Ulid;

impl Database {
    /// Ends the job of `lease` in `to`, succeeded, failed or cancelled, with
    /// `outcome`, the JSON its holder reports, and returns the job as it
    /// ended, without a lease. An outcome is final: the job never moves
    /// again, and after a failure or a cancellation its key starts a new
    /// attempt.
    ///
    /// # Errors
    ///
    /// [`Error::Lost`] when `lease` is no longer the job's, taken over or
    /// ended; [`Error::IllegalMove`] when `to` is not an outcome;
    /// [`Error::UnknownJob`]; and [`Error::Store`].
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
            end(transaction, lease.job, to, outcome)
        })
    }

    /// Cancels job `id` while it is queued, with `outcome`, the JSON that
    /// tells why, and returns it as it ended. A running job is cancelled by
    /// its holder, through [`Database::complete_job`].
    ///
    /// # Errors
    ///
    /// [`Error::Held`], naming its holder, when the job runs;
    /// [`Error::IllegalMove`] when it has ended; [`Error::UnknownJob`]; and
    /// [`Error::Store`].
    pub fn cancel_job(&self, id: Ulid, outcome: &Value) -> Result<Job, Error> {
        self.write(|transaction| {
            let job = find(transaction, id)?.ok_or(Error::UnknownJob(id))?;
            if let Some(current) = job.lease {
                return Err(Error::Held {
                    job: id,
                    holder: current.holder,
                    expires: current.expires,
                });
            }
            if !job.state.may_move_to(JobState::Cancelled) {
                return Err(Error::IllegalMove {
                    job: id,
                    from: job.state,
                    to: JobState::Cancelled,
                });
            }
            end(transaction, id, JobState::Cancelled, outcome)
        })
    }
}

/// Ends job `id` in `to` with `outcome` inside `transaction`: its lease goes
/// and its outcome is recorded. Returns the job as it ended.
fn end(
    transaction: &Transaction<'_>,
    id: Ulid,
    to: JobState,
    outcome: &Value,
) -> Result<Job, Error> {
    let job = transaction.query_row(
        &format!(
            "UPDATE jobs
             SET state = ?2, outcome_json = ?3, lease_holder = NULL, lease_heartbeat = NULL,
               lease_expires = NULL
             WHERE id = ?1
             RETURNING {COLUMNS}"
        ),
        params![id.to_string(), to.as_str(), outcome.to_string()],
        job_row,
    )?;
    Ok(job)
}
