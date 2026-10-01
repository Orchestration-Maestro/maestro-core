//! Durable checkpoints reuse the frontier; complete snapshots never replace them.
use super::{
    capture::Captures,
    envelope::CaptureEnvelope,
    frontier::enqueue_on,
    lease,
    lease::SourceLease,
    partition_record::{AcceptedPartition, Batch, Enumeration, PartitionState},
    privacy,
    privacy::{Handle, ReceiptError},
    record::{COLUMNS, item_row},
};
use crate::{artifact::Digest, scope::Scope, store::Database};
use rusqlite::{OptionalExtension as _, params};
use std::{collections::BTreeSet, time::SystemTime};

/// Replaceable kernel partition boundary over the existing frontier.
pub trait Partitions: Send + Sync {
    /// Atomically enqueue every eligible request before retaining a checkpoint.
    /// # Errors
    /// Invalid bounds, stale ownership, replay/cursor conflict or storage failure.
    fn checkpoint(
        &self,
        writer: &SourceLease,
        batch: &Batch,
        now: SystemTime,
    ) -> Result<(), ReceiptError>;
    /// Accept only a fully covered window with verified captures for every item.
    /// # Errors
    /// Incomplete coverage, pending/failed items, stale ownership or storage failure.
    fn commit_partition(
        &self,
        writer: &SourceLease,
        partition: Handle,
        now: SystemTime,
    ) -> Result<(), ReceiptError>;
    /// Read protected evidence in an exact already-authorized collection scope.
    /// # Errors
    /// Corrupt checkpoint artifacts or storage failure.
    fn partition(
        &self,
        scope: &Scope,
        partition: Handle,
    ) -> Result<Option<PartitionState>, ReceiptError>;
}
impl Partitions for Database {
    fn checkpoint(
        &self,
        writer: &SourceLease,
        batch: &Batch,
        now: SystemTime,
    ) -> Result<(), ReceiptError> {
        validate(batch)?;
        let scope = held(self, writer, now)?;
        let previous = self.partition(&scope, batch.partition.id)?;
        let source: Option<String> = self
            .reader()?
            .query_row(
                "SELECT source FROM acquisition_partitions WHERE id = ?1",
                [batch.partition.id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if source.is_some_and(|source| source != writer.source) {
            return Err(ReceiptError::Conflict);
        }
        if replay(previous.as_ref(), batch)? {
            return Ok(());
        }
        let encoded = serde_json::to_vec(batch)?;
        let references: Vec<_> = batch.capture.into_iter().collect();
        privacy::validate(&scope, &encoded, &references)?;
        if let Some(capture) = batch.capture {
            parent(self, &scope, writer, capture, batch)?;
        }
        let digest = self.put(&encoded, "application/json")?;
        let cursor = Digest::of(&serde_json::to_vec(&batch.cursor)?);
        let sequence = u32::try_from(previous.as_ref().map_or(0, |state| state.batches.len()))
            .map_err(|_| ReceiptError::Invalid)?;
        self.write(|tx| {
            lease::held(tx, writer, now).map_err(|_| ReceiptError::Conflict)?;
            tx.execute(
                "INSERT INTO acquisition_partitions (id, source, scope) VALUES (?1,
                ?2, ?3) ON CONFLICT DO NOTHING",
                params![
                    batch.partition.id.to_string(),
                    writer.source,
                    scope.as_str()
                ],
            )?;
            let (source, stored_scope, count): (String, String, u32) = tx.query_row(
                "SELECT source, scope, (SELECT count(*) FROM
                    acquisition_partition_batches WHERE partition = ?1) FROM
                    acquisition_partitions WHERE id = ?1",
                [batch.partition.id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            if source != writer.source || stored_scope != scope.as_str() || count != sequence {
                return Err(ReceiptError::Conflict);
            }
            for item in &batch.items {
                enqueue_on(tx, writer, &item.request, scope.as_str())
                    .map_err(|_| ReceiptError::Storage)?;
            }
            let evidence = privacy::retain_on(tx, &scope, &digest, &references)?;
            tx.execute(
                "INSERT INTO acquisition_partition_batches (partition, sequence,
                cursor, evidence) VALUES (?1, ?2, ?3, ?4)",
                params![
                    batch.partition.id.to_string(),
                    sequence,
                    cursor.as_str(),
                    evidence.to_string()
                ],
            )?;
            Ok(())
        })
    }
    fn commit_partition(
        &self,
        writer: &SourceLease,
        partition: Handle,
        now: SystemTime,
    ) -> Result<(), ReceiptError> {
        let scope = held(self, writer, now)?;
        let state = self
            .partition(&scope, partition)?
            .ok_or(ReceiptError::Invalid)?;
        let last = state.batches.last().ok_or(ReceiptError::Invalid)?;
        complete(&state)?;
        let mut references = self.batch_handles(partition)?;
        for batch in &state.batches {
            for discovered in &batch.items {
                let reader = self.reader()?;
                let item = reader.query_row(
                    &format!(
                        "SELECT {COLUMNS} FROM acquisition_frontier WHERE source = ?1 AND
                    fetch_identity = ?2 AND authorization_context = ?3 AND
                    representation_profile = ?4"
                    ),
                    params![
                        writer.source,
                        discovered.request.fetch_identity,
                        discovered.request.authorization_context.as_str(),
                        discovered.request.representation_profile.as_str()
                    ],
                    item_row,
                )?;
                let capture: Option<String> = reader
                    .query_row(
                        "SELECT envelope FROM acquisition_capture_links WHERE item = ?1",
                        [item.id.to_string()],
                        |row| row.get(0),
                    )
                    .optional()?;
                let handle: Handle = capture.ok_or(ReceiptError::Conflict)?.parse()?;
                self.verify_capture(&scope, &item, handle)
                    .map_err(|_| ReceiptError::Conflict)?;
                references.push(handle);
            }
        }
        if state.accepted.is_some() {
            return Ok(());
        }
        let snapshot = AcceptedPartition {
            watermark: last.partition.window.end,
        };
        let encoded = serde_json::to_vec(&snapshot)?;
        references.sort_unstable();
        references.dedup();
        privacy::validate(&scope, &encoded, &references)?;
        let digest = self.put(&encoded, "application/json")?;
        self.write(|tx| {
            lease::held(tx, writer, now).map_err(|_| ReceiptError::Conflict)?;
            let source: String = tx.query_row(
                "SELECT source FROM acquisition_partitions WHERE id = ?1",
                [partition.to_string()],
                |row| row.get(0),
            )?;
            if source != writer.source {
                return Err(ReceiptError::Invalid);
            }
            let evidence = privacy::retain_on(tx, &scope, &digest, &references)?;
            tx.execute(
                "INSERT INTO acquisition_partition_snapshots (partition, evidence)
                VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                params![partition.to_string(), evidence.to_string()],
            )?;
            Ok(())
        })
    }
    fn partition(
        &self,
        scope: &Scope,
        partition: Handle,
    ) -> Result<Option<PartitionState>, ReceiptError> {
        let reader = self.reader()?;
        let found: bool = reader.query_row(
            "SELECT EXISTS (SELECT 1 FROM acquisition_partitions WHERE id = ?1 AND scope = ?2)",
            params![partition.to_string(), scope.as_str()],
            |row| row.get(0),
        )?;
        if !found {
            return Ok(None);
        }
        let batches = self
            .batch_handles(partition)?
            .into_iter()
            .map(|handle| {
                serde_json::from_slice(&privacy::snapshot(self, &handle.to_string())?)
                    .map_err(ReceiptError::from)
            })
            .collect::<Result<Vec<Batch>, _>>()?;
        let accepted: Option<String> = reader
            .query_row(
                "SELECT evidence FROM acquisition_partition_snapshots WHERE partition = ?1",
                [partition.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let accepted = accepted
            .map(|handle| {
                serde_json::from_slice(&privacy::snapshot(self, &handle)?)
                    .map_err(ReceiptError::from)
            })
            .transpose()?;
        let source: String = reader.query_row(
            "SELECT source FROM acquisition_partitions WHERE id = ?1",
            [partition.to_string()],
            |row| row.get(0),
        )?;
        let pending = self.pending_items(&source, &batches)?;
        Ok(Some(PartitionState {
            batches,
            pending,
            accepted,
        }))
    }
}
impl Database {
    /// Current distinct-item count, not a checkpoint cursor or a claim of coverage.
    fn pending_items(&self, source: &str, batches: &[Batch]) -> Result<u16, ReceiptError> {
        let reader = self.reader()?;
        let mut seen = BTreeSet::new();
        let mut pending = 0;
        for item in batches.iter().flat_map(|batch| &batch.items) {
            let key = (
                &item.request.fetch_identity,
                &item.request.authorization_context,
                &item.request.representation_profile,
            );
            if !seen.insert(key) {
                continue;
            }
            let captured: bool = reader.query_row(
                "SELECT EXISTS (SELECT 1 FROM acquisition_frontier f
                 JOIN acquisition_capture_links l ON l.item = f.id
                 JOIN acquisition_evidence e ON e.id = l.envelope
                 WHERE f.source = ?1 AND f.fetch_identity = ?2
                 AND f.authorization_context = ?3 AND f.representation_profile = ?4
                 AND f.capture = e.artifact)",
                params![
                    source,
                    item.request.fetch_identity,
                    item.request.authorization_context.as_str(),
                    item.request.representation_profile.as_str()
                ],
                |row| row.get(0),
            )?;
            if !captured {
                pending += 1;
            }
        }
        Ok(pending)
    }
    /// Immutable checkpoint handles, ordered by their validated continuation chain.
    fn batch_handles(&self, partition: Handle) -> Result<Vec<Handle>, ReceiptError> {
        let reader = self.reader()?;
        let mut query = reader.prepare(
            "SELECT evidence FROM acquisition_partition_batches WHERE partition = ?1
            ORDER BY sequence",
        )?;
        let rows = query
            .query_map([partition.to_string()], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(|text| text.parse()).collect()
    }
}
/// Source lease and collection binding remain kernel authority, not adapter state.
fn held(db: &Database, writer: &SourceLease, now: SystemTime) -> Result<Scope, ReceiptError> {
    db.write(|tx| lease::held(tx, writer, now).map_err(|_| ReceiptError::Conflict))?
        .parse()
        .map_err(|_| ReceiptError::Invalid)
}
/// Prepared capture linkage must belong to this source and exact item contexts.
fn parent(
    db: &Database,
    scope: &Scope,
    writer: &SourceLease,
    capture: Handle,
    batch: &Batch,
) -> Result<(), ReceiptError> {
    let reader = db.reader()?;
    let found: bool = reader.query_row(
        "SELECT EXISTS (SELECT 1 FROM acquisition_capture_links l JOIN
        acquisition_evidence e ON e.id = l.envelope JOIN acquisition_frontier f ON
        f.id = l.item WHERE l.envelope = ?1 AND e.scope = ?2 AND f.source = ?3)",
        params![capture.to_string(), scope.as_str(), writer.source],
        |row| row.get(0),
    )?;
    if !found {
        return Err(ReceiptError::Invalid);
    }
    let envelope: CaptureEnvelope =
        serde_json::from_slice(&privacy::snapshot(db, &capture.to_string())?)?;
    for item in &batch.items {
        if item.request.authorization_context != envelope.authorization_context
            || item.request.representation_profile != envelope.profile
            || item.keys.representation != envelope.artifact
        {
            return Err(ReceiptError::Invalid);
        }
    }
    Ok(())
}
/// Bound all durable inventories and preserve unknown coverage instead of guessing.
fn validate(batch: &Batch) -> Result<(), ReceiptError> {
    let partition = &batch.partition;
    if partition.window.start >= partition.window.end
        || !(1..=1000).contains(&partition.max_batches)
        || !(1..=1000).contains(&partition.max_items)
        || batch.items.len() > usize::from(partition.max_items)
        || batch.denied.len() > 1000
    {
        return Err(ReceiptError::Invalid);
    }
    if partition.kind == Enumeration::Links
        && (batch.capture.is_none()
            || batch
                .extractor
                .as_ref()
                .is_none_or(|text| text.is_empty() || text.len() > 512))
    {
        return Err(ReceiptError::Invalid);
    }
    if batch.terminal == batch.next.is_some() {
        return Err(ReceiptError::Invalid);
    }
    let mut identities = BTreeSet::new();
    for item in &batch.items {
        if item.keys.permissions != item.request.authorization_context
            || !identities.insert((
                &item.request.fetch_identity,
                &item.request.authorization_context,
                &item.request.representation_profile,
            ))
        {
            return Err(ReceiptError::Invalid);
        }
    }
    Ok(())
}
/// Exact replay is idempotent; changed or cyclic continuations are conflicts.
fn replay(previous: Option<&PartitionState>, batch: &Batch) -> Result<bool, ReceiptError> {
    if batch.next.is_some() && batch.next == batch.cursor {
        return Err(ReceiptError::Conflict);
    }
    let Some(state) = previous else {
        return if batch.cursor.is_none() {
            Ok(false)
        } else {
            Err(ReceiptError::Conflict)
        };
    };
    if state.batches.iter().any(|saved| saved == batch) {
        return Ok(true);
    }
    let last = state.batches.last().ok_or(ReceiptError::Invalid)?;
    if state.accepted.is_some()
        || last.terminal
        || last.partition != batch.partition
        || last.next != batch.cursor
        || state.batches.len() >= usize::from(batch.partition.max_batches)
    {
        return Err(ReceiptError::Conflict);
    }
    let mut count = batch.items.len();
    for saved in &state.batches {
        if saved.cursor == batch.cursor || (batch.next.is_some() && batch.next == saved.cursor) {
            return Err(ReceiptError::Conflict);
        }
        count += saved.items.len();
    }
    if count > usize::from(batch.partition.max_items) {
        return Err(ReceiptError::Conflict);
    }
    Ok(false)
}
/// Only terminal, stable, uncapped, count-proven windows can be accepted.
fn complete(state: &PartitionState) -> Result<(), ReceiptError> {
    if state.batches.last().is_none_or(|batch| !batch.terminal) {
        return Err(ReceiptError::Conflict);
    }
    for batch in &state.batches {
        if !batch.stable
            || batch.truncated
            || batch.expected.map(usize::from) != Some(batch.items.len())
        {
            return Err(ReceiptError::Conflict);
        }
    }
    Ok(())
}
