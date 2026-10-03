//! Full declared inventories are independent of this run's remaining HTTP slots.
use super::{
    controls::{CaptureWork, SourceWork, storage},
    output::{Entry, Report},
};
use crate::failure::Failure;
use maestro_acquisition::discovery::{links::DomLinks, partition::discover_with_captured};
use maestro_kernel::acquisition::{
    CaptureContext, CaptureEnvelope, Captures, Enumeration, Frontier, Handle, Partition,
    Partitions, Receipts, Window,
};
#[cfg(test)]
use std::cell::Cell;
use std::collections::BTreeSet;

/// Only prioritization reads these markers; reuse still verifies immutable scoped captures.
pub(super) fn captured_children<S: Frontier, T>(
    work: &SourceWork<'_, S, T>,
) -> Result<BTreeSet<String>, Failure> {
    #[cfg(test)]
    SCANS.with(|count| count.set(count.get() + 1));
    let mut captured = BTreeSet::new();
    let mut after = None;
    loop {
        let page = work
            .store
            .page(work.principal.scopes, &work.source.id, after, 1000)
            .map_err(|_| storage())?;
        if page.is_empty() {
            break;
        }
        #[cfg(test)]
        SCAN_ITEMS.with(|count| count.set(count.get() + page.len()));
        after = page.last().map(|item| item.id);
        captured.extend(
            page.into_iter()
                .filter(|item| item.capture.is_some())
                .map(|item| item.request.fetch_identity),
        );
    }
    Ok(captured)
}
/// The sole N13 enumerator acknowledges only stable bounded discovery.
pub(crate) async fn discovery<S, T>(
    work: &mut CaptureWork<'_, '_, S, T>,
    capture: (&CaptureContext, Handle),
    envelope: &CaptureEnvelope,
    depth: u64,
    report: &mut Report,
) -> Result<bool, Failure>
where
    S: Frontier + Captures + Partitions + Receipts,
{
    if !work.source.source.discovery.is_empty() && envelope.declared_media.is_none() {
        report.pending.push(Entry::new(
            envelope.requested.as_str(),
            "discovery_media_unknown",
        ));
        return Ok(false);
    }
    if envelope.declared_media.as_deref() != Some("text/html")
        || work.source.source.discovery.is_empty()
    {
        work.source
            .store
            .acknowledge_capture(capture.0, capture.1)
            .map_err(|_| storage())?;
        return Ok(true);
    }
    if work.partitions.len() as u64 >= work.accounting.limits().partitions.get() {
        report.pending.push(Entry::new(
            envelope.requested.as_str(),
            "inventory_or_partition_limit",
        ));
        return Ok(false);
    }
    let partition = Partition {
        id: Handle::new(),
        run: work.source.receipt.run,
        kind: Enumeration::Links,
        window: Window {
            start: 0,
            end: 1,
            overlap: 0,
            skew: 0,
        },
        max_batches: 1,
        max_items: u16::try_from(work.source.source.limits.pages.get().min(999))
            .map_err(|_| storage())?,
    };
    let extractor = DomLinks::new().map_err(|_| storage())?;
    let discovered = discover_with_captured(
        work.source.store,
        &extractor,
        capture,
        work.source.policy,
        (partition, depth, work.captured_children),
    )
    .await;
    let Ok(batch) = discovered else {
        report.pending.push(Entry::new(
            envelope.requested.as_str(),
            "discovery_unavailable",
        ));
        return Ok(false);
    };
    work.partitions.push((batch.partition.id, depth));
    for excluded in &batch.not_enqueued {
        report.not_enqueued(excluded).map_err(|_| storage())?;
    }
    if batch.inventory_overflow > 0 {
        report.overflow = report
            .overflow
            .checked_add(u64::from(batch.inventory_overflow))
            .ok_or_else(storage)?;
    }
    if !batch.stable || batch.truncated {
        report.pending.push(Entry::new(
            envelope.requested.as_str(),
            "discovery_incomplete",
        ));
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
thread_local! {
    /// Complete captured-child traversals on this test thread.
    pub(super) static SCANS: Cell<usize> = const { Cell::new(0) };
    /// Frontier rows visited by the captured-child traversals.
    pub(super) static SCAN_ITEMS: Cell<usize> = const { Cell::new(0) };
}
