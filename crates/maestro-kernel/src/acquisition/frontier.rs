//! Replaceable frontier contract and the kernel SQLite adapter.
use super::{
    envelope::CaptureEnvelope,
    error::Error,
    lease::{self, DispatchRequest, ItemLease, LeaseRequest, SourceLease},
    record::{COLUMNS, Item, NewItem, item_row, validate},
    work_order::{self, WorkCursor, WorkItem},
};
use crate::{
    artifact::Digest,
    job::{
        events::record_on_stream,
        lease::{expire_source, times},
        unsigned,
    },
    scope::{Scope, ScopeSet},
    store::{Database, artifacts::pin},
};
use rusqlite::{OptionalExtension as _, Transaction, params};
use serde_json::json;
use std::time::{Duration, Instant, SystemTime};
use ulid::Ulid;

/// Durable acquisition queue. Callers depend on this port, not a concrete store.
///
/// Every write validates a current writer, with durable and monotonic expiry.
/// Handles supplied by a connector are untrusted; adapters must recheck their
/// source, holder and both fencing epochs. No transport runs inside a write.
pub trait Frontier: Send + Sync {
    /// Page pending work by ID, then verified work by oldest observation and ID.
    /// # Errors
    /// Invalid bounds or unreadable authorized storage refuse.
    fn work_page(
        &self,
        scopes: &ScopeSet,
        source: &str,
        after: Option<WorkCursor>,
        limit: u16,
    ) -> Result<Vec<WorkItem>, Error>;
    /// Begin a fenced refresh generation without modifying old capture evidence.
    /// # Errors
    /// Lost ownership, unknown/foreign work, or pending work without prepared
    /// evidence refuses. A live same-epoch pending dispatch cannot be refreshed.
    fn refresh(&self, writer: &SourceLease, item: Ulid, now: SystemTime) -> Result<(), Error>;
    /// Expire the exact source writer and its pending dispatch leases.
    /// Items, fencing epochs, attempts and the source job remain durable.
    /// # Errors
    /// Expired or taken-over writers return Lost and cannot release a successor.
    fn release_source(&self, writer: &SourceLease, now: SystemTime) -> Result<(), Error>;
    /// Takes exclusive source ownership, or takes over after its expiry.
    ///
    /// # Errors
    /// Invalid identity/bounds/time, held ownership, or a store failure.
    fn lease_source(
        &self,
        source: &str,
        scope: &Scope,
        request: LeaseRequest<'_>,
    ) -> Result<SourceLease, Error>;
    /// Durably inserts a request/context pair, or returns its existing item.
    ///
    /// # Errors
    /// Invalid identity, expired/fenced source ownership, or a store failure.
    fn enqueue(
        &self,
        writer: &SourceLease,
        request: &NewItem,
        now: SystemTime,
    ) -> Result<Item, Error>;
    /// Dispatches one item under an independent fencing epoch. A writer takeover
    /// makes earlier dispatches eligible again, without losing their attempts.
    /// Zero attempt ceilings refuse; exhausted work remains visible but cannot dispatch.
    ///
    /// # Errors
    /// Invalid bounds, expired/fenced ownership, unavailable work, or store failure.
    fn lease(
        &self,
        writer: &SourceLease,
        item: Ulid,
        request: DispatchRequest<'_>,
    ) -> Result<ItemLease, Error>;
    /// Acknowledges an existing verified artifact under both current epochs.
    /// Equal replay is idempotent; a different digest never overwrites a capture.
    /// This is not the capture-envelope/staging admission operation.
    ///
    /// # Errors
    /// Expired/fenced dispatch, conflicting replay, missing/corrupt artifact, or store failure.
    fn acknowledge(
        &self,
        writer: &SourceLease,
        item: &ItemLease,
        artifact: &Digest,
        now: SystemTime,
    ) -> Result<(), Error>;
    /// Reads at most `limit` items (1–1,000), in opaque ID order after `after`.
    /// Unknown sources and scopes not covered by `scopes` return no rows.
    /// A missing capture means unfinished work, never a successful outcome.
    ///
    /// # Errors
    /// Invalid page bound or a store failure.
    fn page(
        &self,
        scopes: &ScopeSet,
        source: &str,
        after: Option<Ulid>,
        limit: u16,
    ) -> Result<Vec<Item>, Error>;
}

impl Frontier for Database {
    fn work_page(
        &self,
        scopes: &ScopeSet,
        source: &str,
        after: Option<WorkCursor>,
        limit: u16,
    ) -> Result<Vec<WorkItem>, Error> {
        work_order::page(self, scopes, source, after, limit)
    }
    fn refresh(&self, writer: &SourceLease, item: Ulid, now: SystemTime) -> Result<(), Error> {
        work_order::refresh(self, writer, item, now)
    }
    fn release_source(&self, writer: &SourceLease, now: SystemTime) -> Result<(), Error> {
        self.write(|tx| {
            lease::held(tx, writer, now)?;
            let (at, _) = times(tx, now, Duration::ZERO)?;
            tx.execute(
                "UPDATE acquisition_frontier SET lease_expires = ?3
                 WHERE source = ?1 AND writer_epoch = ?2 AND capture IS NULL",
                params![
                    writer.source,
                    i64::try_from(writer.epoch).map_err(|_| Error::Lost)?,
                    at
                ],
            )?;
            expire_source(tx, writer.token, now)?;
            Ok(())
        })
    }
    fn lease_source(
        &self,
        source: &str,
        scope: &Scope,
        request: LeaseRequest<'_>,
    ) -> Result<SourceLease, Error> {
        lease::source(self, source, scope, request)
    }
    fn enqueue(
        &self,
        writer: &SourceLease,
        request: &NewItem,
        now: SystemTime,
    ) -> Result<Item, Error> {
        validate(request)?;
        self.write(|tx| {
            let scope = lease::held(tx, writer, now)?;
            enqueue_on(tx, writer, request, &scope)
        })
    }
    fn lease(
        &self,
        writer: &SourceLease,
        item: Ulid,
        request: DispatchRequest<'_>,
    ) -> Result<ItemLease, Error> {
        if request.max_attempts == 0 {
            return Err(Error::Invalid);
        }
        let deadline = lease::bounds(request.lease)?;
        self.write(|tx| {
            if deadline <= Instant::now() {
                return Err(Error::Lost);
            }
            let scope = lease::held(tx, writer, request.lease.now)?;
            let (at, expires) = times(tx, request.lease.now, request.lease.term)?;
            let update = "UPDATE acquisition_frontier SET attempts = attempts + 1,
                lease_epoch = lease_epoch + 1, writer_epoch = ?3,
                lease_holder = ?4, lease_expires = ?5
                WHERE id = ?1 AND source = ?2 AND capture IS NULL AND attempts < ?7
                AND (lease_epoch = 0 OR lease_expires <= ?6 OR writer_epoch <> ?3)
                RETURNING lease_epoch";
            let epoch = tx
                .query_row(
                    update,
                    params![
                        item.to_string(),
                        writer.source,
                        i64::try_from(writer.epoch).map_err(|_| Error::Lost)?,
                        request.lease.holder,
                        expires,
                        at,
                        request.max_attempts
                    ],
                    |row| unsigned(row, 0),
                )
                .optional()?
                .ok_or(Error::Unavailable)?;
            record_on_stream(
                tx,
                writer.token,
                &scope.parse().map_err(|_| Error::Invalid)?,
                "maestro.acquisition.leased.v1",
                &json!({"item": item.to_string(), "epoch": epoch}),
            )?;
            Ok(ItemLease {
                item,
                source_epoch: writer.epoch,
                epoch,
                holder: request.lease.holder.to_owned(),
                deadline,
            })
        })
    }
    fn acknowledge(
        &self,
        writer: &SourceLease,
        item: &ItemLease,
        artifact: &Digest,
        now: SystemTime,
    ) -> Result<(), Error> {
        // Verify bytes outside the write; pinning inside it refuses a concurrent collection.
        let bytes = self.get(artifact)?;
        let observed = serde_json::from_slice::<CaptureEnvelope>(&bytes)
            .ok()
            .filter(|envelope| {
                envelope.item.to_string() == item.item.to_string()
                    && envelope.source == writer.source
            })
            .map(|envelope| i64::try_from(envelope.observed_ms).map_err(|_| Error::Invalid))
            .transpose()?;
        self.write(|tx| {
            let (scope, captured) = lease::dispatched(tx, writer, item, now)?;
            match captured {
                Some(previous) if previous == artifact.as_str() => return Ok(()),
                Some(_) => return Err(Error::Conflict),
                None => {}
            }
            pin(tx, artifact)?;
            tx.execute(
                "UPDATE acquisition_frontier SET capture = ?2, observed_ms = ?3 WHERE id = ?1",
                params![item.item.to_string(), artifact.as_str(), observed],
            )?;
            record_on_stream(
                tx,
                writer.token,
                &scope.parse().map_err(|_| Error::Invalid)?,
                "maestro.acquisition.acknowledged.v1",
                &json!({"item": item.item.to_string()}),
            )?;
            Ok(())
        })
    }
    fn page(
        &self,
        scopes: &ScopeSet,
        source: &str,
        after: Option<Ulid>,
        limit: u16,
    ) -> Result<Vec<Item>, Error> {
        if limit == 0 || limit > 1000 {
            return Err(Error::Invalid);
        }
        let reader = self.reader()?;
        let mut statement = reader.prepare(&format!(
            "SELECT {COLUMNS} FROM acquisition_frontier
            WHERE source = ?1 AND (?2 IS NULL OR id > ?2) AND job IN
            (SELECT id FROM jobs WHERE {}) ORDER BY id LIMIT ?4",
            ScopeSet::condition("jobs.scope", 3)
        ))?;
        let rows = statement
            .query_map(
                params![
                    source,
                    after.map(|id| id.to_string()),
                    scopes.parameter(),
                    limit
                ],
                item_row,
            )?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}

/// Existing uniqueness/journal transaction reused by partition checkpoints.
pub(super) fn enqueue_on(
    tx: &Transaction<'_>,
    writer: &SourceLease,
    request: &NewItem,
    scope: &str,
) -> Result<Item, Error> {
    validate(request)?;
    let insert = "INSERT INTO acquisition_frontier
                (id, source, job, fetch_identity, authorization_context, representation_profile)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT (source, fetch_identity, authorization_context, representation_profile)
                DO NOTHING";
    let inserted = tx.execute(
        insert,
        params![
            Ulid::generate().to_string(),
            writer.source,
            writer.token.to_string(),
            request.fetch_identity,
            request.authorization_context.as_str(),
            request.representation_profile.as_str()
        ],
    )?;
    let select = format!(
        "SELECT {COLUMNS} FROM acquisition_frontier WHERE
                source = ?1 AND fetch_identity = ?2 AND authorization_context = ?3
                AND representation_profile = ?4"
    );
    let item = tx.query_row(
        &select,
        params![
            writer.source,
            request.fetch_identity,
            request.authorization_context.as_str(),
            request.representation_profile.as_str()
        ],
        item_row,
    )?;
    if inserted != 0 {
        record_on_stream(
            tx,
            writer.token,
            &scope.parse().map_err(|_| Error::Invalid)?,
            "maestro.acquisition.enqueued.v1",
            &json!({"item": item.id.to_string()}),
        )?;
    }
    Ok(item)
}
