//! One source writer; frontier pages, never a second authoritative queue.
use super::{
    controls::{SourceWork, decision, request, storage},
    output::{Entry, Report},
    resources,
    sync_budget::RunBudget,
    sync_discovery::captured_children,
    sync_drain::{Drain, drain},
    sync_window::{coverage, verification},
};
use crate::failure::Failure;
use maestro_acquisition::policy::limits::Limits;
use maestro_acquisition::{
    Principal,
    policy::{format_time, identity::FetchIdentity},
    transport::{
        budget::{Usage, compose},
        connect::PinnedTransport,
        stream::Accounting,
    },
};
use maestro_kernel::{
    acquisition::{
        CaptureEnvelope, Captures, DepthPage, Frontier, InventoryPage, InventorySchema, Item,
        ItemDisposition, LeaseRequest, NewItem, PartitionSummary, Partitions, Receipts,
        SourceLease, Stage, StageItem,
    },
    artifact::Digest,
};
use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry as MapEntry},
    time::{Duration, SystemTime},
};
use tokio::io::{AsyncRead, AsyncWrite};
use ulid::Ulid;

/// A bounded disposition retains both identity and scoped capture evidence.
pub(crate) struct SourceResult {
    /// Capture inventory pages for the same frontier.
    pub(crate) pages: Vec<InventoryPage>,
    /// Authoritative frontier state after the run.
    pub(crate) items: Vec<Item>,
    /// Measured received bytes, not synthesized content size.
    pub(crate) response_bytes: u64,
    /// Admitted retained staging allocation.
    pub(crate) staging_bytes: u64,
}
/// A current writer can finish or expire; it must never be stolen.
pub(super) const SOURCE_OWNED: &str = "acquisition source already owned";
/// Read every authorized frontier page, preserving its exclusive cursor.
fn frontier_items(
    store: &dyn Frontier,
    principal: &Principal<'_>,
    source: &str,
    page_size: u16,
) -> Result<Vec<Item>, Failure> {
    let mut items = Vec::new();
    let mut after = None;
    loop {
        let page = Frontier::page(store, principal.scopes, source, after, page_size)
            .map_err(|_| storage())?;
        if page.is_empty() {
            break;
        }
        after = page.last().map(|item| item.id);
        items.extend(page);
    }
    Ok(items)
}
/// Enqueue selected seeds durably; never fetch a current policy denial.
fn seeds<S: Frontier + Receipts, T>(
    work: &SourceWork<'_, S, T>,
    writer: &SourceLease,
    report: &mut Report,
) -> Result<BTreeMap<Digest, u64>, Failure> {
    let now = SystemTime::now();
    let text = format_time(now).map_err(|_| storage())?;
    let mut depths = BTreeMap::new();
    for url in &work.source.seeds {
        let reason = decision(
            work.policy,
            &request(&work.source.id, url, &text),
            work.runtime.controls,
        );
        if reason == "policy_denial" || reason == "unresolved_identity" || reason == "authority" {
            let entry = Entry::new(url, reason);
            if reason == "policy_denial" {
                report.discarded.push(entry);
            } else {
                report.pending.push(entry);
            }
            continue;
        }
        let identity = FetchIdentity::parse(work.source, url).map_err(|_| storage())?;
        let item = NewItem {
            fetch_identity: identity.as_str().into(),
            authorization_context: Digest::of(work.runtime.kernel_principal.as_bytes()),
            representation_profile: work.source.acquisition_profile.digest.clone(),
        };
        work.store
            .enqueue(writer, &item, now)
            .map_err(|_| storage())?;
        depths.insert(Digest::of(item.fetch_identity.as_bytes()), 0);
    }
    Ok(depths)
}
/// Recover only scoped immutable parent provenance; historical unknown depths remain held.
fn recover<S: Partitions + Receipts, T>(
    work: &SourceWork<'_, S, T>,
    depths: &mut BTreeMap<Digest, u64>,
) -> Result<Vec<PartitionSummary>, Failure> {
    recover_depths(work, depths)?;
    let mut summaries = Vec::new();
    let mut after = None;
    loop {
        let page = work
            .store
            .summary_page(work.scope, &work.source.id, after, 1000)
            .map_err(|_| storage())?;
        if page.is_empty() {
            break;
        }
        after = page.last().map(|summary| (summary.id, summary.sequence));
        summaries.extend(page);
    }
    Ok(summaries)
}
/// Verify each first-provenance parent once, independent of historical inventory size.
fn recover_depths<S: Partitions + Receipts, T>(
    work: &SourceWork<'_, S, T>,
    depths: &mut BTreeMap<Digest, u64>,
) -> Result<(), Failure> {
    let mut after = None;
    let authorization = Digest::of(work.runtime.kernel_principal.as_bytes());
    let mut verified = BTreeMap::new();
    loop {
        let page = work
            .store
            .depth_page(&DepthPage {
                scope: work.scope,
                source: &work.source.id,
                authorization: &authorization,
                profile: &work.source.acquisition_profile.digest,
                after: after.as_deref(),
                limit: 1000,
            })
            .map_err(|_| storage())?;
        if page.is_empty() {
            break;
        }
        after = page.last().map(|item| item.request.fetch_identity.clone());
        for item in page {
            if let MapEntry::Vacant(entry) = verified.entry(item.capture) {
                let bytes = work
                    .store
                    .read(work.runtime.kernel_principal, item.capture)
                    .map_err(|_| storage())?
                    .ok_or_else(storage)?;
                let envelope: CaptureEnvelope =
                    serde_json::from_slice(bytes.bytes()).map_err(|_| storage())?;
                entry.insert(
                    envelope.source == work.source.id
                        && envelope.profile == work.source.acquisition_profile.digest
                        && envelope.authorization_context == authorization,
                );
            }
            if verified.get(&item.capture) == Some(&true) {
                depths
                    .entry(Digest::of(item.request.fetch_identity.as_bytes()))
                    .or_insert(item.depth);
            }
        }
    }
    Ok(())
}
/// Retry only unaccepted, eligible historical parents; accepted history needs no reread.
fn reconcile_checkpoints<S: Partitions, T>(
    work: &SourceWork<'_, S, T>,
    writer: &SourceLease,
    summaries: &[PartitionSummary],
) {
    for summary in summaries {
        if summary.capture.is_some()
            && summary.committable
            && !summary.accepted
            && summary.context.as_ref()
                == Some(&(
                    Digest::of(work.runtime.kernel_principal.as_bytes()),
                    work.source.acquisition_profile.digest.clone(),
                ))
        {
            let _ = work
                .store
                .commit_partition(writer, summary.id, SystemTime::now());
        }
    }
}
/// Compose source and remaining aggregate envelopes before any dispatch.
fn bounds<S, T>(work: &SourceWork<'_, S, T>, run: &RunBudget) -> Result<(Limits, bool), Failure> {
    let remaining = run.remaining(&work.policy.policy().aggregate_limits);
    let exhausted = remaining.is_none();
    let limits = resources::bounds(
        &compose(&[
            work.source.limits.clone(),
            remaining.unwrap_or_else(|| work.policy.policy().aggregate_limits.clone()),
        ])
        .map_err(|_| storage())?,
        false,
    )
    .map_err(|_| storage())?;
    Ok((limits, exhausted))
}
/// Drain current work once per attempt, preserving every failed item in the frontier.
pub(crate) async fn execute<S, T>(
    work: &mut SourceWork<'_, S, T>,
    report: &mut Report,
    run: &mut RunBudget,
) -> Result<SourceResult, Failure>
where
    S: Frontier + Captures + Partitions + Receipts,
    T: PinnedTransport,
    T::Connection: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (limits, exhausted) = bounds(work, run)?;
    let now = SystemTime::now();
    let holder = work.receipt.attempt.to_string();
    let lease = LeaseRequest {
        holder: &holder,
        now,
        term: Duration::from_millis(limits.elapsed_ms.get()).min(Duration::from_hours(1)),
    };
    let writer = work
        .store
        .lease_source(&work.source.id, work.scope, lease)
        .map_err(|_| Failure::refused(SOURCE_OWNED))?;
    work.writers.push(writer.clone());
    let pending_before = report.pending.len();
    let mut depths = seeds(work, &writer, report)?;
    let checkpoints = recover(work, &mut depths)?;
    let (coverage_watermark, window, due_window) = verification(work, &checkpoints)?;
    let captured_children = captured_children(work)?;
    let bounds = [limits.clone()];
    let usage = Usage {
        cpu_millicores: if exhausted {
            0
        } else {
            limits.cpu_millicores.get()
        },
        memory_bytes: if exhausted {
            0
        } else {
            limits.memory_bytes.get()
        },
        ..Usage::default()
    };
    let reservation = run.reserve(work.resources, usage);
    let aggregate_bounds = run.bounds.clone();
    let mut state = Drain {
        watermark: coverage_watermark,
        due_window,
        window,
        depths,
        captured_children,
        exhausted,
        reservation,
        accounting: Accounting::new(limits.clone()),
        usage,
        carried_staging: run.staging,
        aggregate_bounds: &aggregate_bounds,
        seen: BTreeSet::new(),
        attempted: 0,
        inventory: BTreeMap::new(),
        keys: BTreeMap::new(),
        partitions: vec![],
    };
    drain(work, &writer, &bounds, &mut state, report).await?;
    for (partition, _) in &state.partitions {
        if work
            .store
            .commit_partition(&writer, *partition, SystemTime::now())
            .is_err()
        {
            report
                .pending
                .push(Entry::new(&partition.to_string(), "partition_incomplete"));
        }
    }
    reconcile_checkpoints(work, &writer, &checkpoints);
    let items = frontier_items(
        work.store,
        work.principal,
        &work.source.id,
        work.runtime.frontier_page_size,
    )?;
    let complete = report.pending.len() == pending_before;
    coverage(work, &writer, &items, &mut state, (complete, report))?;
    if let Some(reservation) = state.reservation.take() {
        run.reservation = Some(reservation);
    }
    run.record(
        state.attempted,
        &state.accounting,
        state.usage.staging_bytes,
        state.partitions.len() as u64,
    )?;
    let pages = inventory_pages(work, state.inventory)?;
    Ok(SourceResult {
        pages,
        items,
        response_bytes: state.accounting.wire_bytes(),
        staging_bytes: state.usage.staging_bytes,
    })
}
/// Retain bounded pages; partition metadata is protected even when no links exist.
fn inventory_pages<S: Receipts, T>(
    work: &SourceWork<'_, S, T>,
    inventory: BTreeMap<Ulid, StageItem>,
) -> Result<Vec<InventoryPage>, Failure> {
    let partition = work
        .store
        .retain(
            work.scope,
            &serde_json::to_vec(&work.source.id).map_err(|_| storage())?,
            &[],
        )
        .map_err(|_| storage())?;
    let complete = inventory.values().all(|item| {
        item.disposition == ItemDisposition::Accepted
            || item.disposition == ItemDisposition::Unchanged
            || item.disposition == ItemDisposition::Denied
    });
    let entries: Vec<_> = inventory.into_values().collect();
    Ok(entries
        .chunks(999)
        .map(|items| InventoryPage {
            schema: InventorySchema::V1,
            partition,
            stage: Stage::Capture,
            complete,
            items: items.to_vec(),
        })
        .collect())
}
