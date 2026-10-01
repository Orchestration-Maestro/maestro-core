//! N11 accounting before kernel-owned immutable capture preparation.
use super::outcome::{Outcome, reconcile};
use crate::{
    CheckedPolicy, lifecycle::resources::Reservation, policy::limits::Limits,
    transport::budget::Usage,
};
use maestro_kernel::{
    acquisition::{
        CaptureContext, CaptureEnvelope, Captures, Handle, InventoryPage, Item, Receipt,
        ReceiptError, Receipts, Status,
    },
    scope::Scope,
};

/// Owned run accounting carried across capture writes and replay.
#[derive(Debug)]
pub struct CaptureBudget<'a> {
    /// Existing N11 ownership, not a second storage budget.
    pub reservation: &'a mut Reservation,
    /// Complete reviewed source/run limits.
    pub bounds: &'a [Limits],
    /// Actual previously retained allocations.
    pub usage: Usage,
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
    budget
        .reservation
        .checkpoint(budget.bounds, usage)
        .map_err(|_| ReceiptError::Conflict)?;
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
    budget
        .reservation
        .checkpoint(budget.bounds, usage)
        .map_err(|_| ReceiptError::Conflict)?;
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
    receipts.finish(&terminal)?;
    Ok(outcome)
}
