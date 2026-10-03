//! Immutable checkpoint summary index and first-in-history traversal provenance.
use super::{
    envelope::CaptureEnvelope,
    lease::SourceLease,
    partition_record::{Batch, Enumeration, Window},
    privacy::{self, Handle, ReceiptError},
    record::NewItem,
};
use crate::{artifact::Digest, scope::Scope, store::Database};
use rusqlite::{Transaction, params};
use std::iter::once;

/// Lightweight per-batch metadata; never loads a historical item inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionSummary {
    /// Opaque partition identity.
    pub id: Handle,
    /// Immutable batch ordinal within its partition.
    pub sequence: u32,
    /// Exact logical verification/discovery run.
    pub run: Handle,
    /// Immutable enumeration contract.
    pub kind: Enumeration,
    /// Exact frozen window.
    pub window: Window,
    /// Final Verification chunk marker.
    pub verification_final: bool,
    /// Separate accepted snapshot exists.
    pub accepted: bool,
    /// Stable, untruncated checkpoint may be retried.
    pub committable: bool,
    /// Prepared parent evidence, if any.
    pub capture: Option<Handle>,
    /// Exact parent authorization and profile from immutable frontier linkage.
    pub context: Option<(Digest, Digest)>,
}
/// Scoped bounded traversal provenance query.
#[derive(Debug)]
pub struct DepthPage<'a> {
    /// Exact source.
    pub source: &'a str,
    /// Already authorized scope.
    pub scope: &'a Scope,
    /// Current effective caller context.
    pub authorization: &'a Digest,
    /// Current representation profile.
    pub profile: &'a Digest,
    /// Exclusive fetch identity cursor.
    pub after: Option<&'a str>,
    /// Page size, 1–1,000.
    pub limit: u16,
}
/// First checkpoint provenance for one durable traversal identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepthEvidence {
    /// Exact child or parent request.
    pub request: NewItem,
    /// Depth assigned by the original history traversal.
    pub depth: u64,
    /// Scoped parent envelope to verify against current effective context.
    pub capture: Handle,
}
/// Derive the summary and first-depth index in the checkpoint transaction.
pub(super) fn record(
    db: &Database,
    tx: &Transaction<'_>,
    ownership: (&SourceLease, &Scope),
    batch: &Batch,
    sequence: u32,
) -> Result<(), ReceiptError> {
    let (writer, scope) = ownership;
    tx.execute(
        "INSERT INTO acquisition_partition_summaries
         (partition, sequence, run, kind, window, verification_final, committable, capture)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            batch.partition.id.to_string(),
            sequence,
            batch.partition.run.to_string(),
            serde_json::to_string(&batch.partition.kind)?,
            serde_json::to_string(&batch.partition.window)?,
            batch.verification_final,
            batch.stable && !batch.truncated,
            batch.capture.map(|id| id.to_string())
        ],
    )?;
    if let (Some(depth), Some(capture)) = (batch.parent_depth, batch.capture) {
        let envelope: CaptureEnvelope =
            serde_json::from_slice(&privacy::snapshot_on(db, tx, &capture.to_string())?)?;
        let parent = NewItem {
            fetch_identity: envelope.requested.as_str().into(),
            authorization_context: envelope.authorization_context,
            representation_profile: envelope.profile,
        };
        let entries = once((&parent, depth)).chain(
            batch
                .items
                .iter()
                .map(|item| (&item.request, depth.saturating_add(1))),
        );
        for (ordinal, (request, depth)) in entries.enumerate() {
            tx.execute(
                "INSERT INTO acquisition_depths (source, scope, fetch_identity,
                 authorization_context, representation_profile, partition, sequence, ordinal,
                 depth, capture) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT (source, scope, authorization_context, representation_profile,
                  fetch_identity) DO UPDATE SET partition = excluded.partition,
                  sequence = excluded.sequence, ordinal = excluded.ordinal,
                  depth = excluded.depth, capture = excluded.capture
                 WHERE (excluded.partition, excluded.sequence, excluded.ordinal) <
                  (acquisition_depths.partition, acquisition_depths.sequence,
                   acquisition_depths.ordinal)",
                params![
                    writer.source,
                    scope.as_str(),
                    request.fetch_identity,
                    request.authorization_context.as_str(),
                    request.representation_profile.as_str(),
                    batch.partition.id.to_string(),
                    sequence,
                    i64::try_from(ordinal).map_err(|_| ReceiptError::Invalid)?,
                    depth.to_string(),
                    capture.to_string()
                ],
            )?;
        }
    }
    Ok(())
}

/// One SQL page over indexed summaries, independent of item count and artifact size.
pub(super) fn summaries(
    db: &Database,
    scope: &Scope,
    source: &str,
    after: Option<(Handle, u32)>,
    limit: u16,
) -> Result<Vec<PartitionSummary>, ReceiptError> {
    if !(1..=1000).contains(&limit) {
        return Err(ReceiptError::Invalid);
    }
    let reader = db.reader()?;
    let mut query = reader.prepare(
        "SELECT s.partition, s.run, s.kind, s.window, s.verification_final,
         a.partition IS NOT NULL, s.committable, s.capture, s.sequence,
         f.authorization_context, f.representation_profile
         FROM acquisition_partition_summaries s JOIN acquisition_partitions p ON p.id = s.partition
         LEFT JOIN acquisition_partition_snapshots a ON a.partition = s.partition
         LEFT JOIN acquisition_capture_links l ON l.envelope = s.capture
         LEFT JOIN acquisition_frontier f ON f.id = l.item
         WHERE p.scope = ?1 AND p.source = ?2 AND (s.partition, s.sequence) > (?3, ?4)
         ORDER BY s.partition, s.sequence LIMIT ?5",
    )?;
    let rows = query
        .query_map(
            params![
                scope.as_str(),
                source,
                after.map_or(String::new(), |(id, _)| id.to_string()),
                after.map_or(-1_i64, |(_, sequence)| i64::from(sequence)),
                limit
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, u32>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(
            |(
                id,
                run,
                kind,
                window,
                verification_final,
                accepted,
                committable,
                capture,
                sequence,
                authorization,
                profile,
            )| {
                Ok(PartitionSummary {
                    id: id.parse()?,
                    sequence,
                    run: run.parse()?,
                    kind: serde_json::from_str(&kind)?,
                    window: serde_json::from_str(&window)?,
                    verification_final,
                    accepted,
                    committable,
                    capture: capture.map(|id| id.parse()).transpose()?,
                    context: authorization
                        .zip(profile)
                        .map(|(authorization, profile)| {
                            Ok::<_, ReceiptError>((
                                Digest::parse(&authorization).map_err(|_| ReceiptError::Invalid)?,
                                Digest::parse(&profile).map_err(|_| ReceiptError::Invalid)?,
                            ))
                        })
                        .transpose()?,
                })
            },
        )
        .collect()
}

/// One index range over distinct effective identities, never over old inventories.
pub(super) fn depths(
    db: &Database,
    page: &DepthPage<'_>,
) -> Result<Vec<DepthEvidence>, ReceiptError> {
    if !(1..=1000).contains(&page.limit) {
        return Err(ReceiptError::Invalid);
    }
    let reader = db.reader()?;
    let mut query = reader.prepare(
        "SELECT fetch_identity, depth, capture FROM acquisition_depths
         WHERE source = ?1 AND scope = ?2 AND authorization_context = ?3
         AND representation_profile = ?4 AND (?5 IS NULL OR fetch_identity > ?5)
         ORDER BY fetch_identity LIMIT ?6",
    )?;
    let rows = query
        .query_map(
            params![
                page.source,
                page.scope.as_str(),
                page.authorization.as_str(),
                page.profile.as_str(),
                page.after,
                page.limit
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(fetch_identity, depth, capture)| {
            Ok(DepthEvidence {
                request: NewItem {
                    fetch_identity,
                    authorization_context: page.authorization.clone(),
                    representation_profile: page.profile.clone(),
                },
                depth: depth.parse().map_err(|_| ReceiptError::Invalid)?,
                capture: capture.parse()?,
            })
        })
        .collect()
}
