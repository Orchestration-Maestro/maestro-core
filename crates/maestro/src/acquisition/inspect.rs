//! Inspect reads authorized durable records only; no policy, authority or transport.
use super::output::{Entry, Report};
use crate::failure::Failure;
use maestro_kernel::acquisition::{Handle, Receipts};

/// Read a terminal summary through the receipt's transitive current scope checks.
pub(crate) fn inspect(
    store: &dyn Receipts,
    principal: &str,
    attempt: Handle,
) -> Result<Report, Failure> {
    let receipt = store
        .inspect(principal, attempt)
        .map_err(|_| Failure::failed("acquisition storage failed"))?
        .ok_or_else(|| Failure::refused("acquisition receipt unavailable"))?;
    let Some(summary) = receipt.downstream.last().copied() else {
        let mut report = Report::new();
        report.run = Some(receipt.run);
        report.receipt = Some(attempt);
        report.status = receipt.status;
        report
            .pending
            .push(Entry::new(&attempt.to_string(), "attempt_not_finalized"));
        return Ok(report);
    };
    let artifact = store
        .read(principal, summary)
        .map_err(|_| Failure::failed("acquisition storage failed"))?
        .ok_or_else(|| Failure::refused("acquisition receipt unavailable"))?;
    let report: Report = serde_json::from_slice(artifact.bytes())
        .map_err(|_| Failure::failed("acquisition summary invalid"))?;
    if report.schema != "maestro-cli/acquisition/1"
        || report.receipt != Some(attempt)
        || report.run != Some(receipt.run)
        || report.status != receipt.status
    {
        return Err(Failure::failed("acquisition summary invalid"));
    }
    Ok(report)
}
