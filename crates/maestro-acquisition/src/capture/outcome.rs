//! Content-free outcomes over N06's authoritative stage inventories.
use maestro_kernel::acquisition::{
    Captures, Handle, InventoryPage, Item, ItemDisposition, Reason, ReceiptError, Stage, StageItem,
    Status,
};
use maestro_kernel::scope::Scope;
use serde::Serialize;
#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

/// One stage's distinct-item counts; stages are never summed as disjoint items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StageCounts {
    /// Stage at which this item is counted.
    pub stage: Stage,
    /// Counting unit, fixed to distinct items rather than attempts.
    pub unit: CountingUnit,
    /// Distinct items in this stage.
    pub items: u64,
    /// Discovered distinct items.
    pub discovered: u64,
    /// Distinct frontier items actually dispatched, in the capture stage only.
    pub attempted: u64,
    /// Accepted distinct items.
    pub accepted: u64,
    /// Verified unchanged distinct items.
    pub unchanged: u64,
    /// Denied distinct items.
    pub denied: u64,
    /// Blocked distinct items.
    pub blocked: u64,
    /// Refused distinct items.
    pub refused: u64,
    /// Withdrawn distinct items.
    pub withdrawn: u64,
    /// Pending distinct items.
    pub pending: u64,
}
/// Explicit machine counting unit, never an implicit attempt/item sum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CountingUnit {
    /// Each item counted once within its named stage.
    DistinctItems,
}
/// Reconciled machine outcome, derived from the inventories retained by N06.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// Only complete is successful completion.
    pub status: Status,
    /// Dispatch attempts separately, not part of distinct-item inventory.
    pub attempts: u64,
    /// Distinct frontier items with at least one dispatch.
    pub attempted_items: u64,
    /// Union of distinct item identities across stages, not their sum.
    pub distinct_items: u64,
    /// Pending item IDs whose partition/disposition evidence is not yet supplied.
    pub pending_without_disposition: Vec<Handle>,
    /// Distinct counts by stage and disposition.
    pub stages: Vec<StageCounts>,
}
impl Outcome {
    /// Successful completion is exact, not merely absence of an exception.
    #[must_use]
    pub fn success(&self) -> bool {
        self.status == Status::Complete
    }
}
/// Reconcile every page independently of dispatch attempts and derive non-success.
/// Callers retain these same pages through N06 and finish its unique receipt.
/// # Errors
/// Duplicate stage items, oversized pages or completed items without evidence refuse.
pub fn reconcile(
    pages: &[InventoryPage],
    frontier: &[Item],
    reason: Reason,
    verification: (&dyn Captures, &Scope),
) -> Result<Outcome, ReceiptError> {
    let mut frontier_ids = BTreeSet::new();
    let mut frontier_index = BTreeMap::new();
    let mut attempted = BTreeSet::new();
    let mut verified = BTreeSet::new();
    let mut attempts = 0_u64;
    for item in frontier {
        let handle = frontier_key(item)?;
        if !frontier_ids.insert(handle) {
            return Err(ReceiptError::Invalid);
        }
        frontier_index.insert(handle, item);
        attempts = attempts
            .checked_add(item.attempts)
            .ok_or(ReceiptError::Invalid)?;
        if item.attempts > 0 {
            attempted.insert(handle);
        }
        if item.capture.is_some() {
            verified.insert(handle);
        }
    }
    let mut seen = BTreeSet::new();
    let mut distinct = frontier_ids.clone();
    let mut discovered = BTreeSet::new();
    let mut captured = BTreeSet::new();
    let mut stages = BTreeMap::new();
    let mut incomplete = false;
    let mut blocked = false;
    let failed = reason == Reason::Transport;
    for page in pages {
        if page.items.len() > 1000 {
            return Err(ReceiptError::Invalid);
        }
        incomplete |= !page.complete;
        let counts = stages
            .entry(page.stage)
            .or_insert_with(|| counts(page.stage));
        for item in &page.items {
            if !seen.insert((page.stage, item.item)) {
                return Err(ReceiptError::Invalid);
            }
            distinct.insert(item.item);
            record(counts, item.disposition);
            if page.stage == Stage::Discovery && item.disposition == ItemDisposition::Discovered {
                discovered.insert(item.item);
            }
            if page.stage == Stage::Capture {
                captured.insert(item.item);
                counts.attempted += u64::from(attempted.contains(&item.item));
            }
            if page.stage == Stage::Capture
                && (item.disposition == ItemDisposition::Accepted
                    || item.disposition == ItemDisposition::Unchanged)
                && !verified.contains(&item.item)
            {
                return Err(ReceiptError::Invalid);
            }
            check_evidence(item.disposition, item.evidence)?;
            verify_evidence(verification, &frontier_index, item)?;
            match item.disposition {
                ItemDisposition::Blocked => blocked = true,
                ItemDisposition::Refused | ItemDisposition::Pending => incomplete = true,
                ItemDisposition::Discovered if page.stage != Stage::Discovery => incomplete = true,
                _ => {}
            }
        }
    }
    let missing: BTreeSet<_> = frontier_ids
        .difference(&captured)
        .chain(discovered.difference(&captured))
        .copied()
        .collect();
    incomplete |= !missing.is_empty();
    if !missing.is_empty() {
        let counts = stages
            .entry(Stage::Capture)
            .or_insert_with(|| counts(Stage::Capture));
        for item in &missing {
            record(counts, ItemDisposition::Pending);
            counts.attempted += u64::from(attempted.contains(item));
        }
    }
    let status = if failed {
        Status::Failed
    } else if blocked || (reason != Reason::None && !incomplete) {
        Status::Blocked
    } else if incomplete {
        Status::Partial
    } else {
        Status::Complete
    };
    Ok(Outcome {
        status,
        attempts,
        attempted_items: attempted.len() as u64,
        distinct_items: distinct.len() as u64,
        pending_without_disposition: missing.into_iter().collect(),
        stages: stages.into_values().collect(),
    })
}
/// Fresh counters with a named stage and counting unit.
fn counts(stage: Stage) -> StageCounts {
    StageCounts {
        stage,
        unit: CountingUnit::DistinctItems,
        items: 0,
        discovered: 0,
        attempted: 0,
        accepted: 0,
        unchanged: 0,
        denied: 0,
        blocked: 0,
        refused: 0,
        withdrawn: 0,
        pending: 0,
    }
}
/// Exactly one disposition for each distinct stage item.
fn record(counts: &mut StageCounts, disposition: ItemDisposition) {
    counts.items += 1;
    match disposition {
        ItemDisposition::Discovered => counts.discovered += 1,
        ItemDisposition::Accepted => counts.accepted += 1,
        ItemDisposition::Unchanged => counts.unchanged += 1,
        ItemDisposition::Denied => counts.denied += 1,
        ItemDisposition::Blocked => counts.blocked += 1,
        ItemDisposition::Refused => counts.refused += 1,
        ItemDisposition::Withdrawn => counts.withdrawn += 1,
        ItemDisposition::Pending => counts.pending += 1,
    }
}
/// Completed/unchanged bytes need a capture; other terminal items need reason evidence.
fn check_evidence(
    disposition: ItemDisposition,
    evidence: Option<Handle>,
) -> Result<(), ReceiptError> {
    if disposition != ItemDisposition::Discovered
        && disposition != ItemDisposition::Pending
        && evidence.is_none()
    {
        return Err(ReceiptError::Invalid);
    }
    Ok(())
}

/// Counts cannot stand in for verification of accepted evidence, at any stage.
fn verify_evidence(
    verification: (&dyn Captures, &Scope),
    frontier: &BTreeMap<Handle, &Item>,
    item: &StageItem,
) -> Result<(), ReceiptError> {
    if item.disposition == ItemDisposition::Accepted
        || item.disposition == ItemDisposition::Unchanged
    {
        let frontier_item = frontier_item(frontier, item.item)?;
        verification.0.verify_capture(
            verification.1,
            frontier_item,
            item.evidence.ok_or(ReceiptError::Invalid)?,
        )?;
    }
    Ok(())
}

/// Format each frontier identity once while building the reconciliation index.
fn frontier_key(item: &Item) -> Result<Handle, ReceiptError> {
    #[cfg(test)]
    KEY_FORMATS.with(|count| count.set(count.get() + 1));
    item.id.to_string().parse()
}
/// Indexed lookup never scans or reformats the frontier for each completed item.
fn frontier_item<'a>(
    frontier: &BTreeMap<Handle, &'a Item>,
    handle: Handle,
) -> Result<&'a Item, ReceiptError> {
    frontier.get(&handle).copied().ok_or(ReceiptError::Invalid)
}
#[cfg(test)]
thread_local! {
    /// Identity formatting work on this test thread, independent of concurrent tests.
    static KEY_FORMATS: Cell<usize> = const { Cell::new(0) };
}
#[cfg(test)]
mod tests {
    use super::{KEY_FORMATS, frontier_item, frontier_key};
    use maestro_kernel::{
        acquisition::{Handle, Item, NewItem},
        artifact::Digest,
    };
    use std::{cell::Cell, collections::BTreeMap};

    #[test]
    fn s6_reconciliation_index_formats_each_identity_only_once() {
        for rows in [10, 40] {
            let items: Vec<_> = (0..rows)
                .map(|_| Item {
                    id: Handle::new().to_string().parse().unwrap(),
                    source: "synthetic".into(),
                    job: Handle::new().to_string().parse().unwrap(),
                    request: NewItem {
                        fetch_identity: "https://example.test/synthetic".into(),
                        authorization_context: Digest::of(b"authority"),
                        representation_profile: Digest::of(b"profile"),
                    },
                    attempts: 0,
                    epoch: 0,
                    capture: None,
                })
                .collect();
            KEY_FORMATS.with(|count| count.set(0));
            let index: BTreeMap<Handle, &Item> = items
                .iter()
                .map(|item| (frontier_key(item).unwrap(), item))
                .collect();
            for (handle, item) in index.iter().rev() {
                assert_eq!(frontier_item(&index, *handle).unwrap().id, item.id);
            }
            assert!(frontier_item(&index, Handle::new()).is_err());
            assert_eq!(
                KEY_FORMATS.with(Cell::get),
                rows,
                "lookups reformatted/scanned identities"
            );
        }
    }
}
