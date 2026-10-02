//! N11 accounting before kernel-owned immutable capture preparation.
use super::outcome::{Outcome, reconcile};
use crate::transport::budget::compose;
use crate::{
    CheckedPolicy, lifecycle::resources::Reservation, policy::limits::Limits,
    transport::budget::Usage,
};
use maestro_kernel::{
    acquisition::{
        CaptureContext, CaptureEnvelope, Captures, Frontier, Handle, InventoryPage, Item, Receipt,
        ReceiptError, Receipts, SourceLease, Status,
    },
    scope::Scope,
};
use std::time::SystemTime;

/// Owned run accounting carried across capture writes and replay.
#[derive(Debug)]
pub struct CaptureBudget<'a> {
    /// Existing N11 ownership, not a second storage budget.
    pub reservation: &'a mut Reservation,
    /// Complete reviewed source/run limits.
    pub bounds: &'a [Limits],
    /// Other completed sources' retained bytes and the full shared aggregate envelope.
    /// None keeps the existing single-source reservation behavior.
    pub carried_staging: Option<(u64, &'a [Limits])>,
    /// Actual previously retained allocations.
    pub usage: Usage,
}
impl CaptureBudget<'_> {
    /// Check source-local costs before charging the shared retained allocation.
    fn checkpoint(&mut self, usage: Usage) -> Result<(), ReceiptError> {
        let local = compose(self.bounds).map_err(|_| ReceiptError::Invalid)?;
        if usage.staging_bytes > local.staging_bytes.get() {
            return Err(ReceiptError::Conflict);
        }
        let (baseline, bounds) = self.carried_staging.unwrap_or((0, self.bounds));
        let owned = Usage {
            staging_bytes: baseline
                .checked_add(usage.staging_bytes)
                .ok_or(ReceiptError::Invalid)?,
            ..usage
        };
        self.reservation
            .checkpoint(bounds, owned)
            .map_err(|_| ReceiptError::Conflict)
    }
}
/// Checkpoint storage resources before retaining verified immutable body/envelope.
/// This returns a durable capture handle, not a successful stage acknowledgment.
/// # Errors
/// Tightened budgets hold; verification/fencing/conflicting replay refuses.
pub fn prepare(
    captures: &dyn Captures,
    policy: &CheckedPolicy,
    context: &CaptureContext,
    capture: (&CaptureEnvelope, &[u8]),
    budget: &mut CaptureBudget<'_>,
) -> Result<Handle, ReceiptError> {
    let (envelope, bytes) = capture;
    let source = policy
        .policy()
        .sources
        .iter()
        .find(|source| source.id == envelope.source)
        .ok_or(ReceiptError::Invalid)?;
    let profile = policy
        .acquisition_profiles()
        .get(&source.acquisition_profile.id)
        .ok_or(ReceiptError::Invalid)?;
    if envelope.profile != source.acquisition_profile.digest
        || envelope.transport != profile.transport
    {
        return Err(ReceiptError::Invalid);
    }
    let before = captures.capture_bytes(envelope)?;
    let new_bytes = captures.check_capture(context, envelope, bytes)?;
    let mut usage = budget.usage;
    usage.staging_bytes = usage
        .staging_bytes
        .checked_add(new_bytes)
        .ok_or(ReceiptError::Invalid)?;
    budget.checkpoint(usage)?;
    let prepared = captures.prepare_capture(context, envelope, bytes, new_bytes);
    // Read back even after failure: artifact writes can outlive a rolled-back link.
    // Unknown readback keeps the whole admitted allocation, never assumes zero.
    let readback = captures.capture_bytes(envelope);
    let retained = readback.map_or(new_bytes, |after| after.saturating_sub(before));
    usage.staging_bytes = budget
        .usage
        .staging_bytes
        .checked_add(retained)
        .ok_or(ReceiptError::Invalid)?;
    budget.usage = usage;
    budget.checkpoint(usage)?;
    readback?;
    Ok(prepared?.handle)
}

/// Retain reconciled N06 inventory pages and finalize its unique frozen receipt.
/// Only the reconciled complete outcome can claim successful completion.
/// # Errors
/// Invalid inventories, missing evidence, changed frozen inputs or terminal replay refuse.
pub fn finish_run(
    receipts: &(impl Receipts + Captures),
    scope: &Scope,
    receipt: &Receipt,
    pages: &[InventoryPage],
    frontier: &[Item],
) -> Result<Outcome, ReceiptError> {
    let (terminal, outcome) = prepare_receipt(receipts, scope, receipt, pages, frontier)?;
    receipts.finish(&terminal)?;
    Ok(outcome)
}
/// Reconcile once and retain one inventory set for a terminal receipt and its summary.
/// # Errors
/// Missing dispositions or invalid scoped capture evidence refuse before finalization.
pub fn prepare_receipt(
    receipts: &(impl Receipts + Captures),
    scope: &Scope,
    receipt: &Receipt,
    pages: &[InventoryPage],
    frontier: &[Item],
) -> Result<(Receipt, Outcome), ReceiptError> {
    let mut outcome = reconcile(pages, frontier, receipt.reason, (receipts, scope))?;
    // A completed capture stage cannot erase a declared lifecycle failure,
    // hold or owned cancellation elsewhere in the same run.
    match receipt.status {
        Status::Partial | Status::Blocked | Status::Failed | Status::Cancelled => {
            outcome.status = receipt.status;
        }
        Status::Pending | Status::Complete => {}
    }
    if !outcome.pending_without_disposition.is_empty() {
        return Err(ReceiptError::Invalid);
    }
    let mut terminal = receipt.clone();
    terminal.status = outcome.status;
    terminal.attempts = outcome.attempts;
    terminal.inventories = pages
        .iter()
        .map(|page| receipts.retain_inventory(scope, page))
        .collect::<Result<_, _>>()?;
    Ok((terminal, outcome))
}

/// Existing inventory and receipt to finalize after all owned processes are reaped.
#[derive(Debug)]
pub struct Cancellation<'a> {
    /// Exact source ownership, never a successor's lease.
    pub writer: &'a SourceLease,
    /// Current trusted authority clock.
    pub now: SystemTime,
    /// Scoped existing run receipt.
    pub scope: &'a Scope,
    /// Unique pending attempt, with its frozen inputs unchanged.
    pub receipt: &'a Receipt,
    /// Truthful existing stage inventories, preserving pending children.
    pub pages: &'a [InventoryPage],
    /// Durable frontier snapshot for distinct-item reconciliation.
    pub frontier: &'a [Item],
}
/// Cancel only this source epoch and finalize its existing receipt without losing work.
/// Owned process reaping must precede this call; client timeout is not cancellation.
/// # Errors
/// Stale ownership, invalid reconciliation or already finalized receipts refuse.
pub fn cancel_run(
    store: &(impl Frontier + Captures + Receipts),
    cancellation: &Cancellation<'_>,
) -> Result<Outcome, ReceiptError> {
    if cancellation.writer.holder != cancellation.receipt.attempt.to_string() {
        return Err(ReceiptError::Conflict);
    }
    let mut terminal = cancellation.receipt.clone();
    terminal.status = Status::Cancelled;
    let (terminal, outcome) = prepare_receipt(
        store,
        cancellation.scope,
        &terminal,
        cancellation.pages,
        cancellation.frontier,
    )?;
    store
        .release_source(cancellation.writer, cancellation.now)
        .map_err(|_| ReceiptError::Conflict)?;
    store.finish(&terminal)?;
    Ok(outcome)
}
