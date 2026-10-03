//! Indexed pending-first verification order and fenced append-only refreshes.
use super::{
    error::Error,
    lease,
    lease::SourceLease,
    record::{COLUMNS, Item, item_row},
};
use crate::{
    job::{lease::times, unsigned},
    scope::ScopeSet,
    store::Database,
};
use rusqlite::params;
use std::time::{Duration, SystemTime};
use ulid::Ulid;

/// Exclusive work cursor; its order is independent of source content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkCursor {
    /// Pending rows precede verified rows.
    pub verified: bool,
    /// Zero for pending; trusted capture observation for verified work.
    pub observed_ms: u64,
    /// Stable tie-breaker across small cursor pages.
    pub id: Ulid,
}

/// Durable frontier item with its frozen work-order cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItem {
    /// Existing authoritative frontier row.
    pub item: Item,
    /// Cursor before any refresh or acknowledgment changes the row's order.
    pub cursor: WorkCursor,
}

/// One indexed authorized range, never a full-history sort.
pub(super) fn page(
    db: &Database,
    scopes: &ScopeSet,
    source: &str,
    after: Option<WorkCursor>,
    limit: u16,
) -> Result<Vec<WorkItem>, Error> {
    if !(1..=1000).contains(&limit) {
        return Err(Error::Invalid);
    }
    let reader = db.reader()?;
    let mut statement = reader.prepare(&format!(
        "SELECT {COLUMNS}, work_observed FROM acquisition_frontier
         WHERE source = ?1 AND (work_verified, work_observed, id) > (?2, ?3, ?4)
         AND job IN (SELECT id FROM jobs WHERE {})
         ORDER BY work_verified, work_observed, id LIMIT ?6",
        ScopeSet::condition("jobs.scope", 5)
    ))?;
    let (verified, observed, id) = after.map_or((-1, 0, String::new()), |cursor| {
        (
            i32::from(cursor.verified),
            cursor.observed_ms,
            cursor.id.to_string(),
        )
    });
    let observed = i64::try_from(observed).map_err(|_| Error::Invalid)?;
    statement
        .query_map(
            params![source, verified, observed, id, scopes.parameter(), limit],
            |row| {
                let item = item_row(row)?;
                let cursor = WorkCursor {
                    verified: item.capture.is_some(),
                    observed_ms: unsigned(row, 9)?,
                    id: item.id,
                };
                Ok(WorkItem { item, cursor })
            },
        )?
        .collect::<Result<_, _>>()
        .map_err(Error::from)
}

/// Clear only the current acknowledgment; old immutable generation links remain.
pub(super) fn refresh(
    db: &Database,
    writer: &SourceLease,
    item: Ulid,
    now: SystemTime,
) -> Result<(), Error> {
    db.write(|tx| {
        lease::held(tx, writer, now)?;
        let (at, _) = times(tx, now, Duration::ZERO)?;
        let changed = tx.execute(
            "UPDATE acquisition_frontier SET capture = NULL,
             capture_generation = capture_generation + 1, lease_expires = ?3
             WHERE id = ?1 AND source = ?2 AND
             (capture IS NOT NULL OR (
                (lease_expires <= ?3 OR writer_epoch <> ?4) AND EXISTS (
                    SELECT 1 FROM acquisition_capture_links l
                    WHERE l.item = acquisition_frontier.id
                    AND l.generation = acquisition_frontier.capture_generation)))",
            params![
                item.to_string(),
                writer.source,
                at,
                i64::try_from(writer.epoch).map_err(|_| Error::Lost)?
            ],
        )?;
        if changed != 1 {
            return Err(Error::Unavailable);
        }
        Ok(())
    })
}
