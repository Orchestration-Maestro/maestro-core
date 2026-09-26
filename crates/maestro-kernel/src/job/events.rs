//! The events of a job: each change and each step, recorded on the job's
//! stream of the journal in the write that makes it.

use super::{error::Error, state::JobState};
use crate::{
    journal::{Event, NewEvent, event::record},
    scope::Scope,
};
use rusqlite::Transaction;
use serde_json::Value;
use ulid::Ulid;

/// The type of the event that records a job's creation, with its frozen
/// inputs.
pub const CREATED: &str = "maestro.job.created.v1";
/// The type of the event that records the first lease of a queued job.
pub const TAKEN: &str = "maestro.job.taken.v1";
/// The type of the event that records the takeover of an expired lease.
pub const TAKEN_OVER: &str = "maestro.job.taken_over.v1";
/// The type of the events that record a job's steps.
pub const PROGRESSED: &str = "maestro.job.progressed.v1";
/// The type of the event that records a job's success, with its outcome.
pub const SUCCEEDED: &str = "maestro.job.succeeded.v1";
/// The type of the event that records a job's failure, with its outcome.
pub const FAILED: &str = "maestro.job.failed.v1";
/// The type of the event that records a job's cancellation, with its
/// outcome.
pub const CANCELLED: &str = "maestro.job.cancelled.v1";

/// The stream of job `id` in the journal, `job/<id>`: its changes and its
/// steps, in order.
#[must_use]
pub fn stream(id: Ulid) -> String {
    format!("job/{id}")
}

/// The type of the event that records a job's move into `state`: its
/// creation, its first lease, or its end. A takeover keeps a job running and
/// records [`TAKEN_OVER`].
pub(super) fn moved_into(state: JobState) -> &'static str {
    match state {
        JobState::Queued => CREATED,
        JobState::Running => TAKEN,
        JobState::Succeeded => SUCCEEDED,
        JobState::Failed => FAILED,
        JobState::Cancelled => CANCELLED,
    }
}

/// Records an event of `r#type` carrying `data` on the stream of job `id`,
/// which is also its subject, in the job's `scope`, inside `transaction`: the
/// write that makes the change or the step it tells of. Returns it as stored.
///
/// # Errors
///
/// [`Error::Store`] when the database cannot record it: the caller returns
/// the error from its write, which then rolls the change back.
pub(super) fn record_on_stream(
    transaction: &Transaction<'_>,
    id: Ulid,
    scope: &Scope,
    r#type: &str,
    data: &Value,
) -> Result<Event, Error> {
    let name = stream(id);
    let event = NewEvent {
        stream: &name,
        r#type,
        subject: &name,
        scope: scope.as_str(),
        data,
    };
    Ok(record(transaction, &event)?)
}
