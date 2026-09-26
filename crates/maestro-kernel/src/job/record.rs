//! Jobs as the kernel records them, their leases, and the calls that submit
//! and read them.

use super::{
    error::Error,
    events::{moved_into, record_on_stream},
    state::JobState,
};
use crate::{
    artifact::Digest,
    scope::{Scope, ScopeSet},
    store::Database,
};
use rusqlite::{Connection, OptionalExtension as _, Row, params, types::Type};
use serde_json::{Value, json};
use std::{error, time::SystemTime};
use ulid::Ulid;

/// The columns of a job, in the order [`job_row`] reads them.
pub(super) const COLUMNS: &str = "id, kind, idempotency_key, attempt, scope, resource, state, \
                                  lease_number, lease_holder, lease_heartbeat, lease_expires, \
                                  outcome_json";

/// A job to submit: work of a kind on the frozen inputs its caller chose, in
/// the scope it works on, and the resource it holds, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewJob<'a> {
    /// What work it is, such as `knowledge.publish`.
    pub kind: &'a str,
    /// What makes it the same work when a command is retried: the caller
    /// chooses them and freezes them before it submits.
    pub inputs: &'a Value,
    /// The scope it works on, its collection's.
    pub scope: &'a Scope,
    /// What it holds exclusively while it is queued or running, such as the
    /// publication of a collection, if anything: a name its caller chose,
    /// which no other job queued or running holds.
    pub resource: Option<&'a str>,
}

/// A job as the kernel records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// Its ID, a ULID naming the millisecond of the caller's clock it was
    /// submitted at.
    pub id: Ulid,
    /// What work it is.
    pub kind: String,
    /// The digest of its kind, its scope and its inputs, which a retried
    /// command gives again.
    pub idempotency_key: Digest,
    /// Its place among the jobs of its key: 1 for the first, one more for
    /// each job submitted after the last one failed or was cancelled.
    pub attempt: u64,
    /// The scope it works on.
    pub scope: Scope,
    /// What it holds exclusively while it is queued or running, if anything.
    pub resource: Option<String>,
    /// Where it is in its life.
    pub state: JobState,
    /// The lease its holder works under, while it runs.
    pub lease: Option<Lease>,
    /// What it ended with, the JSON its last holder or its canceller gave,
    /// once it ended.
    pub outcome: Option<Value>,
}

/// The lease of a running job: who works on it, and until when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    /// The job it holds.
    pub job: Ulid,
    /// Who holds it: a name its holder chose.
    pub holder: String,
    /// Its number among the job's leases: 1 for the first, one more for each
    /// takeover. The job knows its lease by it, whatever its holder's name.
    pub number: u64,
    /// When its holder last renewed it: RFC 3339 in UTC, to the millisecond.
    pub heartbeat: String,
    /// When it expires unless its holder renews it, after which another may
    /// take it over: RFC 3339 in UTC, to the millisecond.
    pub expires: String,
}

impl Database {
    /// Submits `new` at `now`, the time of the caller's clock its ID names,
    /// and returns the job of its key a retried command finds: the one
    /// queued, running or succeeded, as it is, recording nothing. Otherwise
    /// it creates a job, queued, whose attempt follows the last of its key,
    /// and records [`CREATED`](super::CREATED) in the same write, with its
    /// inputs, and with the ID of that last attempt when there is one.
    ///
    /// The key is the SHA-256 digest of the JSON array `[kind, scope,
    /// inputs]`, in which the fields of each object follow the order of their
    /// names, so the same inputs make the same key however the caller built
    /// them, and the same kind and inputs in another scope are another job.
    /// The key comes first: a retried command finds its job even while that
    /// job holds its resource.
    ///
    /// # Errors
    ///
    /// [`Error::ResourceHeld`], naming the job, when another job, queued or
    /// running, holds the resource of `new`; [`Error::Store`] when the
    /// database cannot record the job or its event, or holds a job it cannot
    /// read back. Nothing is then recorded.
    pub fn submit_job(&self, new: &NewJob<'_>, now: SystemTime) -> Result<Job, Error> {
        let key = idempotency_key(new);
        self.write(|transaction| {
            let live = "idempotency_key = ?1 AND state IN ('queued', 'running', 'succeeded')";
            if let Some(job) = first(transaction, live, key.as_str())? {
                return Ok(job);
            }
            free(transaction, new.resource)?;
            let last = "idempotency_key = ?1 ORDER BY attempt DESC LIMIT 1";
            let previous = first(transaction, last, key.as_str())?;
            let job = transaction.query_row(
                &format!(
                    "INSERT INTO jobs (id, kind, idempotency_key, attempt, scope, resource, state)
                     SELECT ?1, ?2, ?3, coalesce(max(attempt), 0) + 1, ?4, ?5, 'queued'
                     FROM jobs WHERE idempotency_key = ?3
                     RETURNING {COLUMNS}"
                ),
                params![
                    Ulid::from_datetime(now).to_string(),
                    new.kind,
                    key.as_str(),
                    new.scope.as_str(),
                    new.resource,
                ],
                job_row,
            )?;
            let data = match previous {
                Some(previous) => json!({
                    "inputs": new.inputs,
                    "previous_attempt": previous.id.to_string(),
                }),
                None => json!({ "inputs": new.inputs }),
            };
            let created = moved_into(JobState::Queued);
            record_on_stream(transaction, job.id, &job.scope, created, &data)?;
            Ok(job)
        })
    }

    /// The job `id`, if it is recorded and `scopes` covers the scope it works
    /// on, as the last commit left it.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read, or holds a job it
    /// cannot read back.
    pub fn job(&self, scopes: &ScopeSet, id: Ulid) -> Result<Option<Job>, Error> {
        Ok(find(&self.reader()?, Some(scopes), id)?)
    }
}

/// The idempotency key of `new`: the digest of the JSON array `[kind, scope,
/// inputs]`, in which `serde_json` writes the fields of each object in the
/// order of their names.
fn idempotency_key(new: &NewJob<'_>) -> Digest {
    let named = json!([new.kind, new.scope.as_str(), new.inputs]);
    Digest::of(named.to_string().as_bytes())
}

/// Refuses `resource` while a job, queued or running, holds it.
///
/// # Errors
///
/// [`Error::ResourceHeld`], naming that job, and [`Error::Store`] when the
/// database cannot be read.
fn free(connection: &Connection, resource: Option<&str>) -> Result<(), Error> {
    let Some(resource) = resource else {
        return Ok(());
    };
    let holding = "resource = ?1 AND state IN ('queued', 'running')";
    match first(connection, holding, resource)? {
        Some(holder) => Err(Error::ResourceHeld {
            resource: resource.to_owned(),
            job: holder.id,
        }),
        None => Ok(()),
    }
}

/// The job `id` that `connection` records, if any and `scopes` covers the
/// scope it works on; with no set, whatever its scope, as a write checks the
/// job it changes as recorded.
pub(super) fn find(
    connection: &Connection,
    scopes: Option<&ScopeSet>,
    id: Ulid,
) -> rusqlite::Result<Option<Job>> {
    connection
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM jobs WHERE id = ?1 AND (?2 IS NULL OR {})",
                ScopeSet::condition("jobs.scope", 2)
            ),
            params![id.to_string(), scopes.map(ScopeSet::parameter)],
            job_row,
        )
        .optional()
}

/// The first job `connection` records of those `clause` selects, if any:
/// SQL that follows `WHERE`, in which `?1` stands for `value`.
fn first(connection: &Connection, clause: &str, value: &str) -> rusqlite::Result<Option<Job>> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM jobs WHERE {clause}"),
            [value],
            job_row,
        )
        .optional()
}

/// The job of a row of [`COLUMNS`]. An ID that is not a ULID, a key that is
/// not a digest, a scope that is not a scope path, a state no job has, a
/// negative number or an outcome serde cannot read is an error, never a
/// guess.
pub(super) fn job_row(row: &Row<'_>) -> rusqlite::Result<Job> {
    let id: String = row.get(0)?;
    let id = Ulid::from_string(&id).map_err(|invalid| unreadable(0, invalid))?;
    let key: String = row.get(2)?;
    let scope: String = row.get(4)?;
    let state: String = row.get(6)?;
    let number = unsigned(row, 7)?;
    let holder: Option<String> = row.get(8)?;
    let heartbeat: Option<String> = row.get(9)?;
    let expires: Option<String> = row.get(10)?;
    let outcome: Option<String> = row.get(11)?;
    Ok(Job {
        id,
        kind: row.get(1)?,
        idempotency_key: Digest::parse(&key).map_err(|invalid| unreadable(2, invalid))?,
        attempt: unsigned(row, 3)?,
        scope: scope.parse().map_err(|invalid| unreadable(4, invalid))?,
        resource: row.get(5)?,
        state: JobState::named(&state)
            .ok_or_else(|| unreadable(6, format!("no job state is named {state:?}")))?,
        lease: holder
            .zip(heartbeat)
            .zip(expires)
            .map(|((holder, heartbeat), expires)| Lease {
                job: id,
                holder,
                number,
                heartbeat,
                expires,
            }),
        outcome: outcome
            .map(|text| serde_json::from_str(&text))
            .transpose()
            .map_err(|invalid| unreadable(11, invalid))?,
    })
}

/// The integer of column `index`, which the jobs table keeps at zero or
/// above.
pub(super) fn unsigned(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

/// The error of the text of column `index`, which `invalid` tells why it
/// cannot be read.
fn unreadable(
    index: usize,
    invalid: impl Into<Box<dyn error::Error + Send + Sync>>,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, invalid.into())
}
