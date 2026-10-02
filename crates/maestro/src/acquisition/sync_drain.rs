//! Paged pending-first traversal over the authoritative frontier.
use super::{
    controls::{CaptureWork, SourceWork, storage},
    output::{Entry, Report},
    sync_capture::capture,
    sync_disposition::exclusion,
};
use crate::failure::Failure;
use maestro_acquisition::{
    lifecycle::{incremental::due, resources::Reservation},
    policy::limits::Limits,
    transport::{budget::Usage, connect::PinnedTransport, stream::Accounting},
};
use maestro_kernel::{
    acquisition::{
        CaptureEnvelope, CaptureLookup, Captures, ChangeKeys, Frontier, Handle, Item,
        ItemDisposition, Partitions, Receipts, SourceLease, StageItem, Window, WorkCursor,
        WorkItem,
    },
    artifact::Digest,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};
use tokio::io::{AsyncRead, AsyncWrite};
use ulid::Ulid;
/// Bounded process-local traversal metadata; the frontier remains authoritative.
pub(super) struct Drain<'a> {
    /// Last completely accepted local verification window.
    pub(super) watermark: Option<u64>,
    /// Frozen source uncertainty margins and run-start upper bound.
    pub(super) window: Window,
    /// Continuations reuse observations at or after the original pending target.
    pub(super) due_window: Window,
    /// Retained storage from already stopped sources.
    pub(super) carried_staging: u64,
    /// Full aggregate allocation envelope, independent of source-local bounds.
    pub(super) aggregate_bounds: &'a [Limits],
    /// Existing scoped checkpoint provenance, including prepared parent evidence.
    pub(super) prepared_handles: BTreeSet<Handle>,
    /// Captured child markers, scanned once and maintained through refresh/acknowledgment.
    pub(super) captured_children: BTreeSet<String>,
    /// Provenance-validated change keys, never reread during coverage.
    pub(super) keys: BTreeMap<Ulid, ChangeKeys>,
    /// No aggregate headroom can dispatch new work.
    pub(super) exhausted: bool,
    /// Seed and newly discovered item depths, not a queue of dispatch requests.
    pub(super) depths: BTreeMap<Digest, u64>,
    /// Owned resource reservation, absent on a truthful budget hold.
    pub(super) reservation: Option<Reservation>,
    /// Cumulative bounded HTTP observations.
    pub(super) accounting: Accounting,
    /// Actual retained storage allocations.
    pub(super) usage: Usage,
    /// Items already attempted by this invocation.
    pub(super) seen: BTreeSet<Ulid>,
    /// Eligible uncaptured items attempted within this run, not historical reused items.
    pub(super) attempted: u64,
    /// Distinct capture dispositions.
    pub(super) inventory: BTreeMap<Ulid, StageItem>,
    /// Durable N13 partition handles and their parent depths.
    pub(super) partitions: Vec<(Handle, u64)>,
}
/// Re-scan bounded frontier pages so newly checkpointed work is never lost.
pub(super) async fn drain<S, T>(
    work: &SourceWork<'_, S, T>,
    writer: &SourceLease,
    bounds: &[Limits],
    state: &mut Drain<'_>,
    report: &mut Report,
) -> Result<(), Failure>
where
    S: Frontier + Captures + Partitions + Receipts,
    T: PinnedTransport,
    T::Connection: AsyncRead + AsyncWrite + Unpin + Send,
{
    loop {
        let before = state.seen.len();
        scan_pass(work, (writer, bounds), state, report).await?;
        // A full pass catches new checkpoint rows even if an insertion sorted behind the cursor.
        if state.seen.len() == before {
            break;
        }
    }
    Ok(())
}
/// One cursor pass, bounded in memory to the existing 1,000-item pages.
async fn scan_pass<S, T>(
    work: &SourceWork<'_, S, T>,
    owned: (&SourceLease, &[Limits]),
    state: &mut Drain<'_>,
    report: &mut Report,
) -> Result<(), Failure>
where
    S: Frontier + Captures + Partitions + Receipts,
    T: PinnedTransport,
    T::Connection: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (writer, bounds) = owned;
    let mut after: Option<WorkCursor> = None;
    loop {
        let page = work
            .store
            .work_page(
                work.principal.scopes,
                &work.source.id,
                after,
                work.runtime.frontier_page_size,
            )
            .map_err(|_| storage())?;
        if page.is_empty() {
            break;
        }
        after = page.last().map(|item| item.cursor);
        let unseen: Vec<_> = page
            .iter()
            .filter(|row| !state.seen.contains(&row.item.id))
            .map(|row| row.item.clone())
            .collect();
        let captures = work
            .store
            .capture_page(work.scope, &unseen)
            .map_err(|_| storage())?;
        for item in page {
            if state.seen.insert(item.item.id) {
                visit(
                    work,
                    (writer, bounds),
                    (&item, captures.get(&item.item.id)),
                    state,
                    report,
                )
                .await?;
            }
        }
    }
    Ok(())
}
/// Visit one durable row at most once; this is metadata, never a second queue.
async fn visit<S, T>(
    work: &SourceWork<'_, S, T>,
    owned: (&SourceLease, &[Limits]),
    current: (&WorkItem, Option<&CaptureLookup>),
    state: &mut Drain<'_>,
    report: &mut Report,
) -> Result<(), Failure>
where
    S: Frontier + Captures + Partitions + Receipts,
    T: PinnedTransport,
    T::Connection: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (writer, bounds) = owned;
    let limits = bounds.first().ok_or_else(storage)?;
    let (row, observed) = current;
    let original = &row.item;
    if let Some(excluded) = exclusion(work, original, report)? {
        state.inventory.insert(original.id, excluded);
        return Ok(());
    }
    let (refreshed, previous) = refresh(work, writer, (row, observed), state)?;
    let item = &refreshed;
    let observed = if previous.is_some() {
        state.captured_children.remove(&item.request.fetch_identity);
        None
    } else {
        observed
    };
    let reusable = item.capture.is_some()
        || observed.is_some_and(|capture| state.prepared_handles.contains(&capture.handle));
    let reference = Digest::of(item.request.fetch_identity.as_bytes());
    let depth = state.depths.get(&reference).copied();
    let previous_partitions = state.partitions.len();
    let held = if !reusable
        && (item.request.authorization_context
            != Digest::of(work.runtime.kernel_principal.as_bytes())
            || item.request.representation_profile != work.source.acquisition_profile.digest)
    {
        Some("context_changed")
    } else if state.exhausted && !reusable {
        Some("aggregate_budget")
    } else if state.attempted >= limits.pages.get() && !reusable {
        Some("page_budget")
    } else {
        None
    };
    let result = if let Some(reason) = held {
        report
            .pending
            .push(Entry::new(&item.request.fetch_identity, reason));
        StageItem {
            item: item.id.to_string().parse().map_err(|_| storage())?,
            disposition: ItemDisposition::Pending,
            evidence: None,
        }
    } else if let Some(reservation) = state.reservation.as_mut() {
        if !reusable {
            state.attempted = state.attempted.checked_add(1).ok_or_else(storage)?;
        }
        let mut capture_work = CaptureWork {
            previous: previous.as_ref(),
            source: work,
            observed,
            prepared_handles: &state.prepared_handles,
            captured_children: &mut state.captured_children,
            keys: &mut state.keys,
            writer,
            reservation,
            bounds,
            carried_staging: state.carried_staging,
            aggregate_bounds: state.aggregate_bounds,
            accounting: &mut state.accounting,
            usage: &mut state.usage,
            partitions: &mut state.partitions,
        };
        capture(&mut capture_work, item, depth, report).await?
    } else {
        report
            .pending
            .push(Entry::new(&item.request.fetch_identity, "budget"));
        StageItem {
            item: item.id.to_string().parse().map_err(|_| storage())?,
            disposition: ItemDisposition::Pending,
            evidence: None,
        }
    };
    state.inventory.insert(item.id, result);
    // N13 already enqueued these items before returning a checkpoint.
    for (partition, parent_depth) in state.partitions.iter().skip(previous_partitions) {
        let checkpoint = work
            .store
            .partition(work.scope, *partition)
            .map_err(|_| storage())?
            .ok_or_else(storage)?;
        for discovered in checkpoint.batches.iter().flat_map(|batch| &batch.items) {
            state
                .depths
                .entry(Digest::of(discovered.request.fetch_identity.as_bytes()))
                .or_insert(parent_depth.saturating_add(1));
        }
    }
    Ok(())
}

/// A due acknowledged row enters a new fenced generation; old evidence stays immutable.
fn refresh<S: Captures + Frontier + Receipts, T>(
    work: &SourceWork<'_, S, T>,
    writer: &SourceLease,
    current: (&WorkItem, Option<&CaptureLookup>),
    state: &Drain<'_>,
) -> Result<(Item, Option<CaptureEnvelope>), Failure> {
    let (row, observed) = current;
    let original = &row.item;
    let revalidate = due(
        work.runtime.mode,
        state.watermark,
        original.capture.as_ref().map(|_| row.cursor.observed_ms),
        &state.due_window,
    );
    let previous = if original.capture.is_some() && revalidate {
        let envelope = observed
            .filter(|capture| capture.acknowledged)
            .ok_or_else(storage)?
            .envelope
            .clone();
        super::sync_keys::observed_keys(&envelope, original)?;
        work.store
            .refresh(writer, original.id, SystemTime::now())
            .map_err(|_| storage())?;
        Some(envelope)
    } else {
        None
    };
    let mut refreshed = original.clone();
    if previous.is_some() {
        refreshed.capture = None;
    }
    Ok((refreshed, previous))
}
