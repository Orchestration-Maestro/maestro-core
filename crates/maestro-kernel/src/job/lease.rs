//! Leases: taken by one holder at a time, taken over once expired, and
//! renewed by heartbeats, which the journal does not record.

use super::{
    error::Error,
    events::{TAKEN_OVER, moved_into, record_on_stream},
    record::{Job, Lease, find, unsigned},
    state::JobState,
};
use crate::store::Database;
use rusqlite::{Connection, Transaction, params};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ulid::Ulid;

impl Database {
    /// Takes the lease of job `id` for `holder` at `now`, the time of the
    /// caller's clock, for `term`, and returns it: a queued job starts
    /// running, and a running job whose lease expired, at `now` or before,
    /// keeps running under its new holder, with the next lease number. The
    /// expiry is the one its holder's lease records, compared with `now`,
    /// never with another clock. The same write records
    /// [`TAKEN`](super::TAKEN) with the lease, or [`TAKEN_OVER`] with the
    /// lease and the one it took over.
    ///
    /// # Errors
    ///
    /// [`Error::Held`], naming the holder and the expiry, when the job's
    /// lease has not expired, whoever asks; [`Error::IllegalMove`] when the
    /// job has ended; [`Error::UnknownJob`]; [`Error::Time`] when `now` or
    /// the expiry falls outside what the kernel records; and [`Error::Store`]
    /// when the database cannot record the lease or its event. Nothing is
    /// then recorded.
    pub fn take_job(
        &self,
        id: Ulid,
        holder: &str,
        now: SystemTime,
        term: Duration,
    ) -> Result<Lease, Error> {
        self.write(|transaction| {
            let job = find(transaction, id)?.ok_or(Error::UnknownJob(id))?;
            let (heartbeat, expires) = times(transaction, now, term)?;
            match &job.lease {
                Some(current) if current.expires > heartbeat => {
                    return Err(Error::Held {
                        job: id,
                        holder: current.holder.clone(),
                        expires: current.expires.clone(),
                    });
                }
                None if !job.state.may_move_to(JobState::Running) => {
                    return Err(Error::IllegalMove {
                        job: id,
                        from: job.state,
                        to: JobState::Running,
                    });
                }
                _ => {}
            }
            let number = transaction.query_row(
                "UPDATE jobs
                 SET state = 'running', lease_number = lease_number + 1, lease_holder = ?2,
                   lease_heartbeat = ?3, lease_expires = ?4
                 WHERE id = ?1
                 RETURNING lease_number",
                params![id.to_string(), holder, heartbeat, expires],
                |row| unsigned(row, 0),
            )?;
            let lease = Lease {
                job: id,
                holder: holder.to_owned(),
                number,
                heartbeat,
                expires,
            };
            let (r#type, data) = match &job.lease {
                Some(previous) => (
                    TAKEN_OVER,
                    json!({ "lease": lease_data(&lease), "previous_lease": lease_data(previous) }),
                ),
                None => (
                    moved_into(JobState::Running),
                    json!({ "lease": lease_data(&lease) }),
                ),
            };
            record_on_stream(transaction, id, &job.scope, r#type, &data)?;
            Ok(lease)
        })
    }

    /// Renews `lease` at `now` for `term`, once the job's row records it:
    /// its heartbeat becomes `now` and its expiry `now` plus `term`. The
    /// journal records no heartbeat. A holder whose lease expired still
    /// renews it, until another takes it over.
    ///
    /// # Errors
    ///
    /// [`Error::Lost`] when `lease` is no longer the job's, taken over or
    /// ended; [`Error::UnknownJob`]; [`Error::Time`]; and [`Error::Store`].
    /// `lease` then stays as it was.
    pub fn heartbeat(
        &self,
        lease: &mut Lease,
        now: SystemTime,
        term: Duration,
    ) -> Result<(), Error> {
        let renewed = self.write(|transaction| {
            held(transaction, lease)?;
            renewal(transaction, lease, now, term)
        })?;
        *lease = renewed;
        Ok(())
    }
}

/// `lease` as the events of its job carry it.
pub(super) fn lease_data(lease: &Lease) -> Value {
    json!({
        "holder": lease.holder,
        "number": lease.number,
        "heartbeat": lease.heartbeat,
        "expires": lease.expires,
    })
}

/// The job of `lease` as `transaction` records it, when `lease` is still its
/// lease: the job runs under a lease of the same number.
///
/// # Errors
///
/// [`Error::Lost`] when another took the lease over or the job ended,
/// [`Error::UnknownJob`], and [`Error::Store`].
pub(super) fn held(transaction: &Transaction<'_>, lease: &Lease) -> Result<Job, Error> {
    let job = find(transaction, lease.job)?.ok_or(Error::UnknownJob(lease.job))?;
    if job
        .lease
        .as_ref()
        .is_some_and(|current| current.number == lease.number)
    {
        Ok(job)
    } else {
        Err(Error::Lost {
            job: lease.job,
            holder: lease.holder.clone(),
            number: lease.number,
        })
    }
}

/// `lease` renewed at `now` for `term` inside `transaction`, which records
/// it in the job's row; the caller checked that it is still the job's.
///
/// # Errors
///
/// [`Error::Time`], and [`Error::Store`] when the row cannot be written.
pub(super) fn renewal(
    transaction: &Transaction<'_>,
    lease: &Lease,
    now: SystemTime,
    term: Duration,
) -> Result<Lease, Error> {
    let (heartbeat, expires) = times(transaction, now, term)?;
    transaction.execute(
        "UPDATE jobs SET lease_heartbeat = ?2, lease_expires = ?3 WHERE id = ?1",
        params![lease.job.to_string(), heartbeat, expires],
    )?;
    Ok(Lease {
        heartbeat,
        expires,
        ..lease.clone()
    })
}

/// The heartbeat and the expiry of a lease taken or renewed at `now` for
/// `term`, as the kernel records them.
fn times(
    connection: &Connection,
    now: SystemTime,
    term: Duration,
) -> Result<(String, String), Error> {
    let heartbeat = timestamp(connection, Some(now))?;
    let expires = timestamp(connection, now.checked_add(term))?;
    Ok((heartbeat, expires))
}

/// `time` as the kernel records times, RFC 3339 in UTC to the millisecond,
/// which SQLite writes as its `strftime` writes the other tables' times: text
/// of one width, which sorts as the times do.
///
/// # Errors
///
/// [`Error::Time`] when there is no `time` or it falls before 1970 or after
/// 9999, and [`Error::Store`] when SQLite cannot be asked.
fn timestamp(connection: &Connection, time: Option<SystemTime>) -> Result<String, Error> {
    let millis = time
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|since| i64::try_from(since.as_millis()).ok());
    let text: Option<String> = connection.query_row(
        "SELECT strftime('%Y-%m-%dT%H:%M:%fZ', ?1 / 1000.0, 'unixepoch')",
        [millis],
        |row| row.get(0),
    )?;
    text.ok_or(Error::Time)
}
