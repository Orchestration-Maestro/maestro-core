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
    let encoded = serde_json::to_vec(envelope)?;
    let mut usage = budget.usage;
    usage.staging_bytes = usage
        .staging_bytes
        .checked_add(bytes.len() as u64)
        .and_then(|value| value.checked_add(encoded.len() as u64))
        .ok_or(ReceiptError::Invalid)?;
    budget
        .reservation
        .checkpoint(budget.bounds, usage)
        .map_err(|_| ReceiptError::Conflict)?;
    let prepared = captures.prepare_capture(context, envelope, bytes);
    // The checkpoint reserved the worst-case write. Release unused/replayed
    // allocation even on a refusal; immutable retained links survive a hold.
    let retained = prepared
        .as_ref()
        .map_or(0, |capture| capture.retained_bytes);
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
    Ok(prepared?.handle)
}

/// Retain reconciled N06 inventory pages and finalize its unique frozen receipt.
/// Only the reconciled complete outcome can claim successful completion.
/// # Errors
/// Invalid inventories, missing evidence, changed frozen inputs or terminal replay refuse.
pub fn finish_run(
    receipts: &dyn Receipts,
    scope: &Scope,
    receipt: &Receipt,
    pages: &[InventoryPage],
    frontier: &[Item],
) -> Result<Outcome, ReceiptError> {
    let mut outcome = reconcile(pages, frontier, receipt.reason)?;
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
