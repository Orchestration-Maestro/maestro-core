//! Durable checkpoints reuse the frontier; complete snapshots never replace them.
use super::{
    lease,
    lease::SourceLease,
    partition_checkpoint::{checkpoint, held},
    partition_history::{self, DepthEvidence, DepthPage, PartitionSummary},
    partition_record::{AcceptedPartition, Batch, PartitionState},
    privacy,
    privacy::{Handle, ReceiptError},
};
use crate::{scope::Scope, store::Database};
use rusqlite::{OptionalExtension as _, params};
use std::{collections::BTreeSet, time::SystemTime};

/// Replaceable kernel partition boundary over the existing frontier.
pub trait Partitions: Send + Sync {
    /// Read immutable batch summaries without historical inventories or pending lookups.
    /// # Errors
    /// Invalid bounds or unreadable scoped storage refuse.
    fn summary_page(
        &self,
        scope: &Scope,
        source: &str,
        after: Option<(Handle, u32)>,
        limit: u16,
    ) -> Result<Vec<PartitionSummary>, ReceiptError>;
    /// Read bounded first-in-history traversal depths for the exact effective context.
    /// # Errors
    /// Invalid bounds or unreadable scoped storage refuse.
    fn depth_page(&self, page: &DepthPage<'_>) -> Result<Vec<DepthEvidence>, ReceiptError>;

    /// Page existing checkpoint identities for current source provenance recovery.
    /// # Errors
    /// Invalid page bounds or unreadable storage refuse.
    fn partition_page(
        &self,
        scope: &Scope,
        source: &str,
        after: Option<Handle>,
        limit: u16,
    ) -> Result<Vec<Handle>, ReceiptError>;
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
    fn summary_page(
        &self,
        scope: &Scope,
        source: &str,
        after: Option<(Handle, u32)>,
        limit: u16,
    ) -> Result<Vec<PartitionSummary>, ReceiptError> {
        partition_history::summaries(self, scope, source, after, limit)
    }
    fn depth_page(&self, page: &DepthPage<'_>) -> Result<Vec<DepthEvidence>, ReceiptError> {
        partition_history::depths(self, page)
    }

    fn partition_page(
        &self,
        scope: &Scope,
        source: &str,
        after: Option<Handle>,
        limit: u16,
    ) -> Result<Vec<Handle>, ReceiptError> {
        if !(1..=1000).contains(&limit) {
            return Err(ReceiptError::Invalid);
        }
        let reader = self.reader()?;
        let mut query = reader.prepare(
            "SELECT id FROM acquisition_partitions WHERE scope = ?1 AND source = ?2
             AND (?3 IS NULL OR id > ?3) ORDER BY id LIMIT ?4",
        )?;
        query
            .query_map(
                params![
                    scope.as_str(),
                    source,
                    after.map(|id| id.to_string()),
                    limit
                ],
                |row| row.get::<_, String>(0),
            )?
            .map(|row| {
                row.map_err(ReceiptError::from)
                    .and_then(|text| text.parse())
            })
            .collect()
    }

    fn checkpoint(
        &self,
        writer: &SourceLease,
        batch: &Batch,
        now: SystemTime,
    ) -> Result<(), ReceiptError> {
        checkpoint(self, writer, batch, now)
    }
    fn commit_partition(
        &self,
        writer: &SourceLease,
        partition: Handle,
        now: SystemTime,
    ) -> Result<(), ReceiptError> {
        self.commit_partition_with(
            writer,
            partition,
            now,
            #[cfg(test)]
            || {},
        )
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
                serde_json::from_slice(&privacy::snapshot_on(self, &reader, &handle.to_string())?)
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
                serde_json::from_slice(&privacy::snapshot_on(self, &reader, &handle)?)
                    .map_err(ReceiptError::from)
            })
            .transpose()?;
        let source: String = reader.query_row(
            "SELECT source FROM acquisition_partitions WHERE id = ?1",
            [partition.to_string()],
            |row| row.get(0),
        )?;
        let pending = self.pending_items(scope, &source, &batches)?;
        Ok(Some(PartitionState {
            batches,
            pending,
            accepted,
        }))
    }
}
impl Database {
    /// Test seam after the initial read, before fenced snapshot retention.
    fn commit_partition_with(
        &self,
        writer: &SourceLease,
        partition: Handle,
        now: SystemTime,
        #[cfg(test)] between: impl FnOnce(),
    ) -> Result<(), ReceiptError> {
        let scope = held(self, writer, now)?;
        let state = self
            .partition(&scope, partition)?
            .ok_or(ReceiptError::Invalid)?;
        let last = state.batches.last().ok_or(ReceiptError::Invalid)?;
        complete(&state)?;
        let mut references = self.batch_handles(partition)?;
        let reader = self.reader()?;
        let source: String = reader.query_row(
            "SELECT source FROM acquisition_partitions WHERE id = ?1",
            [partition.to_string()],
            |row| row.get(0),
        )?;
        if source != writer.source {
            return Err(ReceiptError::Invalid);
        }
        for batch in &state.batches {
            for handle in super::partition_captures::bound(self, &scope, &source, &batch.items)? {
                references.push(handle.ok_or(ReceiptError::Conflict)?);
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
        #[cfg(test)]
        between();
        self.write(|tx| {
            lease::held(tx, writer, now).map_err(|_| ReceiptError::Conflict)?;
            // A partition's source never changes, and it was checked against this writer above.
            let accepted: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM acquisition_partition_snapshots
                 WHERE partition = ?1)",
                [partition.to_string()],
                |row| row.get(0),
            )?;
            if accepted {
                return Ok(());
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
    /// Current distinct-item count, not a checkpoint cursor or a claim of coverage.
    fn pending_items(
        &self,
        scope: &Scope,
        source: &str,
        batches: &[Batch],
    ) -> Result<u16, ReceiptError> {
        let mut pending = BTreeSet::new();
        for batch in batches {
            let handles = super::partition_captures::bound(self, scope, source, &batch.items)?;
            for (item, _) in batch
                .items
                .iter()
                .zip(handles)
                .filter(|(_, handle)| handle.is_none())
            {
                pending.insert((
                    &item.request.fetch_identity,
                    &item.request.authorization_context,
                    &item.request.representation_profile,
                ));
            }
        }
        u16::try_from(pending.len()).map_err(|_| ReceiptError::Invalid)
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
/// Only terminal, stable, uncapped, count-proven windows can be accepted.
fn complete(state: &PartitionState) -> Result<(), ReceiptError> {
    if state.batches.last().is_none_or(|batch| !batch.terminal) {
        return Err(ReceiptError::Conflict);
    }
    for batch in &state.batches {
        if !batch.stable
            || batch
                .not_enqueued
                .iter()
                .any(|entry| entry.reason.pending())
            || batch.truncated
            || batch.expected.map(usize::from) != Some(batch.items.len())
        {
            return Err(ReceiptError::Conflict);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Partitions, privacy::Handle};
    use crate::{
        acquisition::{Batch, Enumeration, Frontier, LeaseRequest, Partition, Window},
        scope::Scope,
        store::Database,
    };
    use maestro_test_scratch::scratch_directory;
    use std::{
        cell::Cell,
        fs,
        time::{Duration, UNIX_EPOCH},
    };

    #[test]
    fn n13_review_concurrent_commit_retains_no_orphan_evidence() {
        let root = scratch_directory().unwrap();
        let db = Database::open_in(&root).unwrap();
        let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
        let now = UNIX_EPOCH + Duration::from_secs(1000);
        let writer = db
            .lease_source(
                "notes",
                &scope,
                LeaseRequest {
                    holder: "worker",
                    now,
                    term: Duration::from_secs(30),
                },
            )
            .unwrap();
        let batch = Batch {
            partition: Partition {
                id: Handle::new(),
                run: Handle::new(),
                kind: Enumeration::Index,
                window: Window {
                    start: 10,
                    end: 20,
                    overlap: 0,
                    skew: 0,
                },
                max_batches: 1,
                max_items: 1,
            },
            cursor: None,
            next: None,
            terminal: true,
            verification_final: false,
            stable: true,
            truncated: false,
            expected: Some(0),
            items: vec![],
            extractor: None,
            parent_keys: None,
            not_enqueued: vec![],
            inventory_overflow: 0,
            parent_depth: None,
            capture: None,
        };
        db.checkpoint(&writer, &batch, now).unwrap();
        let mut retained = (0, 0);
        db.commit_partition_with(&writer, batch.partition.id, now, || {
            db.commit_partition(&writer, batch.partition.id, now)
                .unwrap();
            retained = counts(&db);
        })
        .unwrap();
        assert_eq!(
            counts(&db),
            retained,
            "concurrent replay retained orphan evidence or pins"
        );
        assert_eq!(
            db.partition(&scope, batch.partition.id)
                .unwrap()
                .unwrap()
                .accepted
                .unwrap()
                .watermark,
            20
        );
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    /// Both opaque evidence count and total artifact pins must be unchanged.
    fn counts(db: &Database) -> (u32, u32) {
        db.reader()
        .unwrap()
        .query_row(
            "SELECT (SELECT count(*) FROM acquisition_evidence), (SELECT sum(pins) FROM artifacts)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn n13_review_append_snapshot_loads_do_not_grow_with_history() {
        let root = scratch_directory().unwrap();
        let db = Database::open_in(&root).unwrap();
        let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
        let now = UNIX_EPOCH + Duration::from_secs(1000);
        let writer = db
            .lease_source(
                "notes",
                &scope,
                LeaseRequest {
                    holder: "worker",
                    now,
                    term: Duration::from_secs(3600),
                },
            )
            .unwrap();
        let mut batch = Batch {
            partition: Partition {
                id: Handle::new(),
                run: Handle::new(),
                kind: Enumeration::Index,
                window: Window {
                    start: 10,
                    end: 20,
                    overlap: 0,
                    skew: 0,
                },
                max_batches: 200,
                max_items: 1,
            },
            cursor: None,
            next: None,
            terminal: false,
            verification_final: false,
            stable: true,
            truncated: false,
            expected: Some(0),
            items: vec![],
            extractor: None,
            parent_keys: None,
            not_enqueued: vec![],
            inventory_overflow: 0,
            parent_depth: None,
            capture: None,
        };
        let mut second = 0;
        for index in 0..200 {
            batch.cursor = (index > 0).then(|| serde_json::json!(index));
            batch.next = Some(serde_json::json!(index + 1));
            super::super::privacy::SNAPSHOT_READS.with(|count| count.set(0));
            db.checkpoint(&writer, &batch, now).unwrap();
            let reads = super::super::privacy::SNAPSHOT_READS.with(Cell::get);
            if index == 1 {
                second = reads;
            }
            if index == 199 {
                assert_eq!(reads, second, "snapshot loads grew with history");
            }
        }
        assert_eq!(second, 1, "an append loads only the last checkpoint");
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
}
