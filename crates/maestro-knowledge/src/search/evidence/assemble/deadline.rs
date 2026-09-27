//! Runs evidence assembly under the handoff's absolute deadline.

use super::super::super::request::EvidenceInput;
use super::super::types::{EvidenceCounter, EvidenceError};
use super::{engine, validate};
use maestro_kernel::{evidence::Bundle, retrieval::ReadControl, store::Database};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    task::spawn_blocking,
    time::{Instant, timeout_at},
};

/// Builds authoritative source passages from the bounded rerank handoff.
///
/// # Errors
/// Returns an error for invalid input, changed authority, failed reads, or timeout.
pub async fn assemble_evidence(
    database: Arc<Database>,
    input: EvidenceInput,
    counter: EvidenceCounter,
) -> Result<Bundle, EvidenceError> {
    if Instant::now() >= input.deadline {
        return Err(EvidenceError::TimedOut);
    }
    validate::validate_input(&input)?;
    if Instant::now() >= input.deadline {
        return Err(EvidenceError::TimedOut);
    }

    let deadline = input.deadline;
    let control = ReadControl {
        deadline: deadline.into_std(),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let _cancellation = CancellationOnDrop(control.cancelled.clone());
    let worker =
        spawn_blocking(move || engine::assemble_blocking(&database, &input, &counter, &control));
    let result = timeout_at(deadline, worker).await;
    if Instant::now() >= deadline {
        return Err(EvidenceError::TimedOut);
    }
    match result {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(EvidenceError::WorkerFailed),
        Err(_) => Err(EvidenceError::TimedOut),
    }
}

/// Sets the blocking worker's stop flag when its async waiter is dropped.
struct CancellationOnDrop(Arc<AtomicBool>);

impl Drop for CancellationOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
