//! Progress: each step of a job recorded on its stream of the journal, in
//! the write that renews its lease, and read back to resume it.

use super::{
    error::Error,
    lease::{held, renewal},
    record::Lease,
};
use crate::{
    journal::{Event, Filter, NewEvent, event::record},
    store::Database,
};
use serde_json::Value;
use std::time::{Duration, SystemTime};
use ulid::Ulid;

/// The type of the events that record a step of a job.
pub const PROGRESSED: &str = "maestro.job.progressed.v1";

/// The stream of job `id` in the journal, `job/<id>`: its steps, in order.
#[must_use]
pub fn stream(id: Ulid) -> String {
    format!("job/{id}")
}

impl Database {
    /// Records `data`, the caller's JSON, as the next step of the job of
    /// `lease`, and renews `lease` at `now` for `term`, as a heartbeat does,
    /// in one write: the event and the renewal are committed together or
    /// not at all, and `lease` is renewed once they are. The event is a
    /// [`PROGRESSED`] on the job's [`stream`], which is also its subject, in
    /// the job's scope.
    ///
    /// # Errors
    ///
    /// As [`Database::heartbeat`]: nothing is recorded, and `lease` stays as
    /// it was.
    pub fn progress(
        &self,
        lease: &mut Lease,
        now: SystemTime,
        term: Duration,
        data: &Value,
    ) -> Result<Event, Error> {
        let name = stream(lease.job);
        let (renewed, event) = self.write(|transaction| {
            let job = held(transaction, lease)?;
            let renewed = renewal(transaction, lease, now, term)?;
            let event = record(
                transaction,
                &NewEvent {
                    stream: &name,
                    r#type: PROGRESSED,
                    subject: &name,
                    scope: &job.scope,
                    data,
                },
            )?;
            Ok::<_, Error>((renewed, event))
        })?;
        *lease = renewed;
        Ok(event)
    }

    /// The last step job `id` recorded, which a process resuming it
    /// continues after; none before its first.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownJob`], [`Error::Store`] when the job cannot be read,
    /// and [`Error::Journal`] when its stream cannot.
    pub fn last_progress(&self, id: Ulid) -> Result<Option<Event>, Error> {
        self.job(id)?.ok_or(Error::UnknownJob(id))?;
        let mut steps = self.events(&Filter {
            stream: &stream(id),
            after: 0,
            r#type: Some(PROGRESSED),
        })?;
        Ok(steps.pop())
    }
}
