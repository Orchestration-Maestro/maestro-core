//! Fencing handles: durable authority time plus a local monotonic deadline.
use super::{error::Error, record::source_id};
use crate::{
    job::{NewJob, lease::times},
    scope::Scope,
    store::Database,
};
use rusqlite::{OptionalExtension as _, Transaction, params};
use serde_json::json;
use std::time::{Duration, Instant, SystemTime};
use ulid::Ulid;

/// A bounded dispatch lease, from the authority's clock, with no hidden default.
#[derive(Debug, Clone, Copy)]
pub struct LeaseRequest<'a> {
    /// Opaque worker identity, nonempty and at most 128 bytes, without NUL.
    pub holder: &'a str,
    /// Current time from the trusted authority clock, not a connector's clock.
    pub now: SystemTime,
    /// Term between one millisecond and one hour inclusive.
    pub term: Duration,
}
/// One item dispatch with an explicit finite ceiling on durable attempts.
#[derive(Debug, Clone, Copy)]
pub struct DispatchRequest<'a> {
    /// Worker, authority time and bounded term for this dispatch.
    pub lease: LeaseRequest<'a>,
    /// Maximum dispatch attempts under the current budget (1–`u32::MAX`).
    /// Reaching it leaves the item unfinished and visible in paged reads.
    pub max_attempts: u32,
}
/// Source ownership handle. Adapters recheck every field against durable state.
#[derive(Debug, Clone)]
pub struct SourceLease {
    /// Bounded source identity.
    pub source: String,
    /// Opaque ownership token; the SQLite adapter uses its source job ID.
    pub token: Ulid,
    /// Current source-writer fencing epoch.
    pub epoch: u64,
    /// Worker identity bound to the epoch.
    pub holder: String,
    /// Local monotonic deadline; moving the authority clock back cannot revive it.
    pub deadline: Instant,
}
/// One dispatched item, bound to both its writer and its independent item epoch.
#[derive(Debug, Clone)]
pub struct ItemLease {
    /// Durable item identity.
    pub item: Ulid,
    /// Source-writer epoch that dispatched it.
    pub source_epoch: u64,
    /// Independent item epoch, incremented on retry.
    pub epoch: u64,
    /// Worker identity bound to this dispatch.
    pub holder: String,
    /// Local monotonic deadline, checked in addition to the durable expiry.
    pub deadline: Instant,
}
/// Validates the caller's explicit lease bounds before acquiring anything.
pub(super) fn bounds(request: LeaseRequest<'_>) -> Result<Instant, Error> {
    if request.term < Duration::from_millis(1)
        || request.term > Duration::from_hours(1)
        || request.holder.is_empty()
        || request.holder.len() > 128
        || request.holder.contains('\0')
    {
        return Err(Error::Invalid);
    }
    Instant::now()
        .checked_add(request.term)
        .ok_or(Error::Invalid)
}
/// Leases the existing source job; its journal and epoch survive restarts.
pub(super) fn source(
    db: &Database,
    name: &str,
    scope: &Scope,
    request: LeaseRequest<'_>,
) -> Result<SourceLease, Error> {
    source_id(name)?;
    let deadline = bounds(request)?;
    // Check authority time before submitting a durable job.
    times(&*db.reader()?, request.now, request.term)?;
    let resource = format!("acquisition/source/{name}");
    let job = db.submit_job(
        &NewJob {
            kind: "acquisition.source",
            inputs: &json!({"source": name}),
            scope,
            resource: Some(&resource),
        },
        request.now,
    )?;
    db.write(|tx| {
        tx.execute(
            "INSERT INTO acquisition_sources (source, job) VALUES (?1, ?2)
            ON CONFLICT (source) DO NOTHING",
            params![name, job.id.to_string()],
        )?;
        let stored: String = tx.query_row(
            "SELECT job FROM acquisition_sources WHERE source = ?1",
            [name],
            |row| row.get(0),
        )?;
        if stored != job.id.to_string() {
            return Err(Error::Lost);
        }
        Ok(())
    })?;
    let lease = db.take_job(job.id, request.holder, request.now, request.term)?;
    Ok(SourceLease {
        source: name.to_owned(),
        token: job.id,
        epoch: lease.number,
        holder: lease.holder,
        deadline,
    })
}
/// Checks source fencing, holder and expiry inside the transaction that uses them.
pub(super) fn held(
    tx: &Transaction<'_>,
    writer: &SourceLease,
    now: SystemTime,
) -> Result<String, Error> {
    if writer.deadline <= Instant::now() {
        return Err(Error::Lost);
    }
    let (at, _) = times(tx, now, Duration::ZERO)?;
    let scope = tx
        .query_row(
            "SELECT jobs.scope FROM acquisition_sources AS sources
        JOIN jobs ON jobs.id = sources.job WHERE sources.source = ?1 AND jobs.id = ?2
        AND jobs.lease_number = ?3 AND jobs.lease_holder = ?4
        AND jobs.lease_expires > ?5",
            params![
                writer.source,
                writer.token.to_string(),
                i64::try_from(writer.epoch).map_err(|_| Error::Lost)?,
                writer.holder,
                at
            ],
            |row| row.get(0),
        )
        .optional()?;
    scope.ok_or(Error::Lost)
}

/// Recheck both dispatch epochs, holder and expiries in the committing transaction.
pub(super) fn dispatched(
    tx: &Transaction<'_>,
    writer: &SourceLease,
    item: &ItemLease,
    now: SystemTime,
) -> Result<(String, Option<String>), Error> {
    if item.deadline <= Instant::now() || item.source_epoch != writer.epoch {
        return Err(Error::Lost);
    }
    let scope = held(tx, writer, now)?;
    let (at, _) = times(tx, now, Duration::ZERO)?;
    let captured: Option<Option<String>> = tx
        .query_row(
            "SELECT capture FROM acquisition_frontier
        WHERE id = ?1 AND source = ?2 AND writer_epoch = ?3 AND lease_epoch = ?4
        AND lease_holder = ?5 AND lease_expires > ?6",
            params![
                item.item.to_string(),
                writer.source,
                i64::try_from(writer.epoch).map_err(|_| Error::Lost)?,
                i64::try_from(item.epoch).map_err(|_| Error::Lost)?,
                item.holder,
                at
            ],
            |row| row.get(0),
        )
        .optional()?;
    Ok((scope, captured.ok_or(Error::Lost)?))
}
