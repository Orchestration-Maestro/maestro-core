//! Runs evidence assembly under the handoff's absolute deadline.

use super::super::super::{deadline::std_deadline, request::EvidenceInput};
use super::super::types::{EvidenceCounter, EvidenceError};
use super::{engine, validate};
use maestro_kernel::{
    evidence::Bundle,
    retrieval::ReadControl,
    store::Database,
    telemetry::{
        span,
        stage::{Carried, Count, Outcome},
    },
};
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
/// The assembly is traced as a `retrieval.assemble` stage, whose phases are
/// its child stages, run on the blocking worker.
///
/// # Errors
/// Returns an error for invalid input, changed authority, failed reads, or timeout.
pub async fn assemble_evidence(
    database: Arc<Database>,
    input: EvidenceInput,
    counter: EvidenceCounter,
) -> Result<Bundle, EvidenceError> {
    let stage = span::assemble();
    stage.collection(&input.generation.collection_id);
    stage.generation(input.generation.id);
    let carried = stage.carry();
    let result = stage
        .instrument(assemble_until_deadline(database, input, counter, carried))
        .await;
    if let Ok(bundle) = &result {
        stage.count(Count::Passages, bundle.passages.len());
    }
    stage.finish(
        result
            .as_ref()
            .map_or_else(EvidenceError::outcome, |_| Outcome::Ok),
    );
    result
}

/// Assembles on a blocking worker, which runs its phases under `carried`,
/// until the handoff's deadline.
async fn assemble_until_deadline(
    database: Arc<Database>,
    input: EvidenceInput,
    counter: EvidenceCounter,
    carried: Carried,
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
        deadline: std_deadline(deadline),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let _cancellation = CancellationOnDrop(control.cancelled.clone());
    let worker = spawn_blocking(move || {
        carried.in_scope(|| engine::assemble_blocking(&database, &input, &counter, &control))
    });
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
