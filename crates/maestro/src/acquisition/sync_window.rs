//! Verification coverage reuses immutable bounded kernel partition checkpoints.
use super::{
    controls::{SourceWork, storage},
    output::{Entry, PartitionLimit, Report},
    sync_drain::Drain,
};
use crate::failure::Failure;
use maestro_acquisition::{
    lifecycle::{full::Mode, incremental::window, resume::same_inputs},
    transport::budget::Usage,
};
use maestro_kernel::acquisition::{
    Batch, ChangeKeys, DiscoveredItem, Enumeration, Handle, Item, ItemDisposition, Partition,
    PartitionSummary, Partitions, Receipts, SourceLease, Status, UnfinalizedPage, Window,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

/// Last accepted bound plus the oldest uncommitted verification target.
pub(crate) struct WindowState {
    /// Conservative local complete source watermark.
    pub(crate) watermark: Option<u64>,
    /// Original pending target, never advanced by a continuation receipt.
    pub(crate) pending: Option<Window>,
}
/// Only accepted local verification partitions are source clock watermarks.
pub(crate) fn watermark<S: Partitions + Receipts, T>(
    work: &SourceWork<'_, S, T>,
    summaries: &[PartitionSummary],
) -> Result<WindowState, Failure> {
    let mut watermark = None;
    let mut runs: BTreeMap<Handle, (Window, bool, bool)> = BTreeMap::new();
    for state in summaries {
        if state.kind == Enumeration::Verification {
            let run = runs
                .entry(state.run)
                .or_insert((state.window.clone(), true, false));
            run.1 &= state.accepted;
            run.2 |= state.verification_final;
        }
    }
    let mut pending = Vec::new();
    for (run, (window, accepted, final_chunk)) in runs {
        if accepted && final_chunk && finalized(work, run)? {
            watermark = Some(watermark.unwrap_or(0).max(window.end));
        } else {
            pending.push(window);
        }
    }
    let pending = pending
        .into_iter()
        .filter(|window| watermark.is_none_or(|accepted| window.end > accepted))
        .min_by_key(|window| window.end);
    Ok(WindowState { watermark, pending })
}

/// An unfinalized receipt cannot publish a partially committed chunk set.
fn finalized<S: Receipts, T>(work: &SourceWork<'_, S, T>, run: Handle) -> Result<bool, Failure> {
    let mut after = None;
    loop {
        let page = Receipts::page(work.store, work.runtime.kernel_principal, run, after, 1000)
            .map_err(|_| storage())?;
        if page.iter().any(|progress| {
            progress.status == Status::Complete || progress.status == Status::Partial
        }) {
            return Ok(true);
        }
        if page.is_empty() {
            return Ok(false);
        }
        after = page.last().map(|progress| progress.receipt);
    }
}

/// Frozen local target and the distinct reuse cutoff for pending-window continuation.
pub(crate) fn verification<S: Partitions + Receipts, T>(
    work: &SourceWork<'_, S, T>,
    summaries: &[PartitionSummary],
) -> Result<(Option<u64>, Window, Window), Failure> {
    let previous = watermark(work, summaries)?;
    let now = u64::try_from(
        work.runtime
            .run_now
            .duration_since(UNIX_EPOCH)
            .map_err(|_| storage())?
            .as_millis(),
    )
    .map_err(|_| storage())?;
    let bounds = window(
        previous.watermark,
        now,
        work.source.sync.overlap_ms,
        work.source.sync.clock_skew_ms,
    )
    .map_err(|_| Failure::refused("acquisition verification clock refused"))?;
    if work.runtime.mode != Mode::Incremental {
        return Ok((previous.watermark, bounds.clone(), bounds));
    }
    let recovered = recover_target(work, previous.watermark)?;
    let pending = previous
        .pending
        .into_iter()
        .chain(recovered)
        .min_by_key(|window| window.end);
    let Some(pending) = pending else {
        return Ok((previous.watermark, bounds.clone(), bounds));
    };
    if pending.end > now {
        return Err(Failure::refused("acquisition verification clock refused"));
    }
    let mut cutoff = pending.clone();
    cutoff.start = pending.end;
    Ok((Some(pending.end), pending, cutoff))
}

/// Actual distinct coverage; an old acknowledged capture cannot excuse pending revalidation.
pub(crate) fn coverage<S: Partitions + Receipts, T>(
    work: &SourceWork<'_, S, T>,
    writer: &SourceLease,
    items: &[Item],
    state: &mut Drain<'_>,
    outcome: (bool, &mut Report),
) -> Result<(), Failure> {
    let (complete, report) = outcome;
    let eligible: Vec<_> = items
        .iter()
        .filter(|item| {
            state
                .inventory
                .get(&item.id)
                .is_none_or(|stage| stage.disposition != ItemDisposition::Denied)
        })
        .collect();
    // Even an empty complete inventory records its conservative run-start watermark.
    let size = usize::from(work.runtime.frontier_page_size.min(999));
    // Drain already validated the explicit work-page bound.
    let chunks = eligible.chunks(size);
    let count = chunks.len().max(1);
    let available = if state.exhausted {
        0
    } else {
        state
            .accounting
            .limits()
            .partitions
            .get()
            .saturating_sub(state.partitions.len() as u64)
    };
    let complete = complete && count as u64 <= available;
    if count as u64 > available {
        let mut entry = Entry::new(&work.source.id, "inventory_or_partition_limit");
        entry.partition_limit = Some(PartitionLimit {
            chunks: count as u64,
            ceiling: state.accounting.limits().partitions.get(),
        });
        report.pending.push(entry);
    }
    for index in 0..count.min(usize::try_from(available).unwrap_or(usize::MAX)) {
        let selected = eligible
            .get(index * size..((index + 1) * size).min(eligible.len()))
            .ok_or_else(storage)?;
        let batch = Batch {
            partition: Partition {
                id: Handle::new(),
                run: work.receipt.run,
                kind: Enumeration::Verification,
                window: state.window.clone(),
                max_batches: 1,
                max_items: 999,
            },
            cursor: None,
            next: None,
            terminal: true,
            verification_final: index + 1 == count,
            stable: complete,
            truncated: false,
            expected: Some(u16::try_from(selected.len()).map_err(|_| storage())?),
            items: selected
                .iter()
                .map(|item| {
                    Ok(DiscoveredItem {
                        request: item.request.clone(),
                        keys: state
                            .keys
                            .get(&item.id)
                            .cloned()
                            .unwrap_or_else(|| ChangeKeys {
                                revision: None,
                                validator: None,
                                metadata: None,
                                permissions: item.request.authorization_context.clone(),
                                links: None,
                                representation: None,
                            }),
                    })
                })
                .collect::<Result<_, Failure>>()?,
            extractor: None,
            parent_keys: None,
            not_enqueued: vec![],
            inventory_overflow: 0,
            parent_depth: None,
            capture: None,
        };
        if !charge(
            state,
            serde_json::to_vec(&batch).map_err(|_| storage())?.len() as u64,
        )? {
            report.pending.push(Entry::new(&work.source.id, "budget"));
            break;
        }
        state.partitions.push((batch.partition.id, 0));
        work.store
            .checkpoint(writer, &batch, SystemTime::now())
            .map_err(|_| storage())?;
        if complete {
            work.store
                .commit_partition(writer, batch.partition.id, SystemTime::now())
                .map_err(|_| storage())?;
        }
    }
    Ok(())
}

/// Reserve retained checkpoint bytes before the kernel can allocate them.
fn charge(state: &mut Drain<'_>, bytes: u64) -> Result<bool, Failure> {
    let staging_bytes = state
        .usage
        .staging_bytes
        .checked_add(bytes)
        .ok_or_else(storage)?;
    if staging_bytes > state.accounting.limits().staging_bytes.get() {
        return Ok(false);
    }
    let owned = Usage {
        staging_bytes: state
            .carried_staging
            .checked_add(staging_bytes)
            .ok_or_else(storage)?,
        ..state.usage
    };
    let Some(reservation) = state.reservation.as_mut() else {
        return Ok(false);
    };
    if reservation
        .checkpoint(state.aggregate_bounds, owned)
        .is_err()
    {
        return Ok(false);
    }
    state.usage.staging_bytes = staging_bytes;
    Ok(true)
}

/// Oldest crashed target under the exact current caller, scope and resource refs.
fn recover_target<S: Receipts, T>(
    work: &SourceWork<'_, S, T>,
    watermark: Option<u64>,
) -> Result<Option<Window>, Failure> {
    let mut after = None;
    loop {
        let page = work
            .store
            .unfinalized(
                work.runtime.kernel_principal,
                &UnfinalizedPage {
                    scope: work.scope,
                    now: SystemTime::now(),
                    after,
                    limit: 1000,
                },
            )
            .map_err(|_| storage())?;
        if page.is_empty() {
            return Ok(None);
        }
        after = page.last().map(|receipt| receipt.attempt);
        for receipt in page {
            if receipt.run == work.receipt.run {
                continue;
            }
            let bytes = work
                .store
                .read(work.runtime.kernel_principal, receipt.inputs)
                .map_err(|_| storage())?
                .ok_or_else(storage)?;
            let Ok(inputs) = serde_json::from_slice::<Value>(bytes.bytes()) else {
                continue;
            };
            if !same_inputs(
                work.store,
                work.runtime.kernel_principal,
                receipt.inputs,
                work.receipt.inputs,
            )
            .map_err(|_| storage())?
            {
                continue;
            }
            let Some(run_now) = inputs.get("run_now") else {
                continue;
            };
            let time: SystemTime =
                serde_json::from_value(run_now.clone()).map_err(|_| storage())?;
            let end = u64::try_from(
                time.duration_since(UNIX_EPOCH)
                    .map_err(|_| storage())?
                    .as_millis(),
            )
            .map_err(|_| storage())?;
            if watermark.is_some_and(|accepted| end <= accepted) {
                continue;
            }
            return window(
                watermark,
                end,
                work.source.sync.overlap_ms,
                work.source.sync.clock_skew_ms,
            )
            .map(Some)
            .map_err(|_| storage());
        }
    }
}
