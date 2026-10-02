//! Constant-evidence checkpoint validation and transactional distinct inventory.
use super::{
    envelope::CaptureEnvelope,
    frontier::enqueue_on,
    lease::{self, SourceLease},
    partition_record::{Batch, Enumeration},
    privacy::{self, ReceiptError},
};
use crate::{artifact::Digest, scope::Scope, store::Database};
use rusqlite::{Connection, OptionalExtension as _, params};
use std::{collections::BTreeSet, time::SystemTime};

/// Append or exactly replay one checkpoint, fenced with its item inventory.
pub(super) fn checkpoint(
    db: &Database,
    writer: &SourceLease,
    batch: &Batch,
    now: SystemTime,
) -> Result<(), ReceiptError> {
    validate(batch)?;
    let scope = held(db, writer, now)?;
    let reader = db.reader()?;
    let cursor = Digest::of(&serde_json::to_vec(&batch.cursor)?);
    let count = previous(db, &reader, writer, batch, &cursor)?;
    let Some(sequence) = count else { return Ok(()) };
    let encoded = serde_json::to_vec(batch)?;
    let references: Vec<_> = batch.capture.into_iter().collect();
    privacy::validate(&scope, &encoded, &references)?;
    let digest = db.put(&encoded, "application/json")?;
    db.write(|tx| {
        lease::held(tx, writer, now).map_err(|_| ReceiptError::Conflict)?;
        if batch.capture.is_some() {
            parent(db, tx, &scope, writer, batch)?;
        }
        tx.execute(
            "INSERT INTO acquisition_partitions (id, source, scope)
             VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING",
            params![
                batch.partition.id.to_string(),
                writer.source,
                scope.as_str()
            ],
        )?;
        let (source, stored_scope, count): (String, String, u32) = tx.query_row(
            "SELECT source, scope, (SELECT coalesce(max(sequence) + 1, 0)
             FROM acquisition_partition_batches WHERE partition = ?1)
             FROM acquisition_partitions WHERE id = ?1",
            [batch.partition.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if source != writer.source || stored_scope != scope.as_str() || count != sequence {
            return Err(ReceiptError::Conflict);
        }
        for item in &batch.items {
            enqueue_on(tx, writer, &item.request, scope.as_str())
                .map_err(|_| ReceiptError::Storage)?;
            tx.execute(
                "INSERT INTO acquisition_partition_items (partition, fetch_identity,
                 authorization_context, representation_profile) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO NOTHING",
                params![
                    batch.partition.id.to_string(),
                    item.request.fetch_identity,
                    item.request.authorization_context.as_str(),
                    item.request.representation_profile.as_str()
                ],
            )?;
        }
        let distinct: u16 = tx.query_row(
            "SELECT count(*) FROM acquisition_partition_items WHERE partition = ?1",
            [batch.partition.id.to_string()],
            |row| row.get(0),
        )?;
        if distinct > batch.partition.max_items {
            return Err(ReceiptError::Conflict);
        }
        let evidence = privacy::retain_on(tx, &scope, &digest, &references)?;
        tx.execute(
            "INSERT INTO acquisition_partition_batches (partition, sequence, cursor, evidence)
             VALUES (?1, ?2, ?3, ?4)",
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

/// Read only matching and last evidence through the same connection.
fn previous(
    db: &Database,
    reader: &Connection,
    writer: &SourceLease,
    batch: &Batch,
    cursor: &Digest,
) -> Result<Option<u32>, ReceiptError> {
    if batch.next.is_some() && batch.next == batch.cursor {
        return Err(ReceiptError::Conflict);
    }
    let source: Option<String> = reader
        .query_row(
            "SELECT source FROM acquisition_partitions WHERE id = ?1",
            [batch.partition.id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if source.is_some_and(|source| source != writer.source) {
        return Err(ReceiptError::Conflict);
    }
    let matched: Option<String> = reader
        .query_row(
            "SELECT evidence FROM acquisition_partition_batches
         WHERE partition = ?1 AND cursor = ?2",
            params![batch.partition.id.to_string(), cursor.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(handle) = matched {
        let saved: Batch = serde_json::from_slice(&privacy::snapshot_on(db, reader, &handle)?)?;
        return if saved == *batch {
            Ok(None)
        } else {
            Err(ReceiptError::Conflict)
        };
    }
    let last: Option<(u32, String)> = reader
        .query_row(
            "SELECT sequence, evidence FROM acquisition_partition_batches
         WHERE partition = ?1 ORDER BY sequence DESC LIMIT 1",
            [batch.partition.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((sequence, handle)) = last else {
        return if batch.cursor.is_none() {
            Ok(Some(0))
        } else {
            Err(ReceiptError::Conflict)
        };
    };
    let saved: Batch = serde_json::from_slice(&privacy::snapshot_on(db, reader, &handle)?)?;
    let accepted: bool = reader.query_row(
        "SELECT EXISTS (SELECT 1 FROM acquisition_partition_snapshots WHERE partition = ?1)",
        [batch.partition.id.to_string()],
        |row| row.get(0),
    )?;
    if accepted
        || saved.terminal
        || saved.partition != batch.partition
        || saved.next != batch.cursor
        || sequence + 1 >= u32::from(batch.partition.max_batches)
    {
        return Err(ReceiptError::Conflict);
    }
    if let Some(next) = &batch.next {
        let digest = Digest::of(&serde_json::to_vec(&Some(next))?);
        let cyclic: bool = reader.query_row(
            "SELECT EXISTS (SELECT 1 FROM acquisition_partition_batches
             WHERE partition = ?1 AND cursor = ?2)",
            params![batch.partition.id.to_string(), digest.as_str()],
            |row| row.get(0),
        )?;
        if cyclic {
            return Err(ReceiptError::Conflict);
        }
    }
    Ok(Some(sequence + 1))
}
/// Prepared capture linkage must belong to this source and exact item contexts.
fn parent(
    db: &Database,
    reader: &Connection,
    scope: &Scope,
    writer: &SourceLease,
    batch: &Batch,
) -> Result<(), ReceiptError> {
    let capture = batch.capture.ok_or(ReceiptError::Invalid)?;
    let found: bool = reader.query_row(
        "SELECT EXISTS (SELECT 1 FROM acquisition_capture_links l JOIN
        acquisition_evidence e ON e.id = l.envelope JOIN acquisition_frontier f ON
        f.id = l.item AND l.generation = f.capture_generation
        WHERE l.envelope = ?1 AND e.scope = ?2 AND f.source = ?3)",
        params![capture.to_string(), scope.as_str(), writer.source],
        |row| row.get(0),
    )?;
    if !found {
        return Err(ReceiptError::Invalid);
    }
    let envelope: CaptureEnvelope =
        serde_json::from_slice(&privacy::snapshot_on(db, reader, &capture.to_string())?)?;
    let keys = batch.parent_keys.as_ref().ok_or(ReceiptError::Invalid)?;
    let validator = Digest::of(&serde_json::to_vec(&envelope.headers)?);
    let metadata = Digest::of(&serde_json::to_vec(&(
        &envelope.declared_media,
        &envelope.detected_media,
    ))?);
    if keys.links.is_none()
        || keys.representation.as_ref() != Some(&envelope.artifact)
        || keys.validator.as_ref() != Some(&validator)
        || keys.metadata.as_ref() != Some(&metadata)
        || keys.permissions != envelope.authorization_context
    {
        return Err(ReceiptError::Invalid);
    }
    for item in &batch.items {
        if item.request.authorization_context != envelope.authorization_context
            || item.request.representation_profile != envelope.profile
            || item.keys.representation.is_some()
            || item.keys.validator.is_some()
            || item.keys.metadata.is_some()
            || item.keys.links.is_some()
        {
            return Err(ReceiptError::Invalid);
        }
    }
    Ok(())
}
/// Bound all durable inventories and preserve unknown coverage instead of guessing.
fn validate(batch: &Batch) -> Result<(), ReceiptError> {
    let partition = &batch.partition;
    if partition.window.start > partition.window.end
        || (partition.window.start == partition.window.end
            && partition.kind != Enumeration::Verification)
        || !(1..=1000).contains(&partition.max_batches)
        || partition.max_items > 1000
        || (partition.kind == Enumeration::Index && partition.max_items == 0)
        || usize::from(partition.max_batches) + usize::from(partition.max_items)
            > privacy::MAX_REFERENCES
        || batch.items.len() > usize::from(partition.max_items)
        || (batch.inventory_overflow > 0 && !batch.truncated)
        || batch.not_enqueued.len() > 1000
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

/// Source lease and collection binding remain kernel authority, not adapter state.
pub(super) fn held(
    db: &Database,
    writer: &SourceLease,
    now: SystemTime,
) -> Result<Scope, ReceiptError> {
    db.write(|tx| lease::held(tx, writer, now).map_err(|_| ReceiptError::Conflict))?
        .parse()
        .map_err(|_| ReceiptError::Invalid)
}
