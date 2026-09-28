//! Absolute cutoffs shared by routes, candidate loading and T032 handoff.

use maestro_kernel::evidence::RequestBudget;
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    task::spawn_blocking,
    time::{self, Instant},
};

/// The reason code of a route or the rerank that ran out of time.
pub const DEADLINE_EXCEEDED: &str = "deadline_exceeded";
/// The stable route status reason when disabled by the search configuration.
pub const DISABLED_BY_CONFIGURATION: &str = "disabled by search configuration";

/// The largest window allowed for independent retrieval routes.
const MAX_ROUTE_WINDOW: Duration = Duration::from_millis(300);
/// The largest portion of the request deadline reserved for T032.
const MAX_T032_RESERVE: Duration = Duration::from_millis(50);

/// The one accepted deadline and its retrieval/T032 phase cutoffs.
#[derive(Debug, Clone, Copy)]
pub(super) struct Deadlines {
    /// The full request deadline passed to T032.
    pub(super) expires: Instant,
    /// The deadline shared by independent retrieval routes.
    pub(super) routes: Instant,
    /// The latest a route's one-time setup, such as loading its model, may
    /// end, and the latest the dense route and the rerank may end: evidence
    /// assembly keeps its measured need, the T032 reserve and two windows
    /// (650 ms at any deadline from 1.5 s; it took 340-500 ms on a real
    /// collection), and setup gets the rest, so cold models load in time.
    pub(super) setup: Instant,
    /// The final retrieval cutoff, before reserving time for T032.
    pub(super) work: Instant,
    /// How long each route may take once it is ready.
    pub(super) window: Duration,
}

impl Deadlines {
    /// The cutoff of a route whose one-time setup ended at `ready`: its
    /// window from then, but never past `setup`, where evidence assembly's
    /// time begins.
    pub(super) fn route_after(&self, ready: Instant) -> Instant {
        (ready + self.window).min(self.setup)
    }

    /// The latest optional reranker enrichment may read sources: one window
    /// before `setup`, so the rerank keeps its time; at most `3/4` of the
    /// request deadline plus its reserve precede it, so it follows the start.
    pub(super) fn enrichment(&self) -> Instant {
        self.setup - self.window
    }
}

/// Derives every phase cutoff once from the accepted request budget.
pub(super) fn from_budget(started: Instant, budget: RequestBudget) -> Deadlines {
    let duration = Duration::from_millis(u64::from(budget.deadline_ms));
    let expires = started + duration;
    let route_window = (duration / 4).min(MAX_ROUTE_WINDOW);
    let reserve = (duration / 10).min(MAX_T032_RESERVE);
    let work = expires - reserve;
    Deadlines {
        expires,
        routes: (started + route_window).min(expires),
        setup: work - route_window * 2,
        work,
        window: route_window,
    }
}

/// A deadline elapsed before its future completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DeadlineElapsed;

/// The failure to join a controlled blocking worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockingFailure {
    /// The absolute cutoff elapsed before the worker returned.
    TimedOut,
    /// The blocking worker panicked or could not be joined.
    WorkerFailed,
}

/// Polls only before `deadline`, returning without polling already-expired work.
pub(super) async fn until<T>(
    deadline: Instant,
    future: impl Future<Output = T>,
) -> Result<T, DeadlineElapsed> {
    if Instant::now() >= deadline {
        return Err(DeadlineElapsed);
    }
    time::timeout_at(deadline, future)
        .await
        .map_err(|_| DeadlineElapsed)
}

/// Runs one blocking reader until its absolute cutoff and cancels it on return.
pub(super) async fn run_blocking<T>(
    deadline: Instant,
    worker: impl FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
) -> Result<T, BlockingFailure>
where
    T: Send + 'static,
{
    if Instant::now() >= deadline {
        return Err(BlockingFailure::TimedOut);
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let guard = CancelOnDrop(cancelled.clone());
    let worker = spawn_blocking(move || worker(cancelled));
    let result = match until(deadline, worker).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Err(BlockingFailure::WorkerFailed),
        Err(DeadlineElapsed) => Err(BlockingFailure::TimedOut),
    };
    drop(guard);
    result
}

/// Sets cancellation once a blocking reader's caller stops waiting.
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
