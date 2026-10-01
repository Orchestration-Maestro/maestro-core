//! Absolute cutoffs shared by routes, candidate loading and T032 handoff.

use maestro_kernel::{
    evidence::RequestBudget,
    retrieval::{Clock, ReadControl},
};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant as StdInstant},
};
use tokio::{
    runtime::Handle,
    task::spawn_blocking,
    time::{self, Instant},
};

/// The reason code of a route or the rerank that ran out of time.
pub const DEADLINE_EXCEEDED: &str = "deadline_exceeded";
/// The stable route status reason when disabled by the search configuration.
pub const DISABLED_BY_CONFIGURATION: &str = "disabled by search configuration";

/// Tokio's runtime clock, including a test runtime's paused clock.
///
/// In production, when time is not paused, Tokio's clock tracks the system
/// monotonic clock. Capturing the handle lets blocking workers read it safely.
#[derive(Clone, Debug)]
pub struct RuntimeClock(Handle);

impl RuntimeClock {
    /// Captures the currently entered runtime for blocking workers.
    ///
    /// # Panics
    ///
    /// When called outside a Tokio runtime.
    #[must_use]
    pub fn current() -> Self {
        Self(Handle::current())
    }
}

impl Clock for RuntimeClock {
    fn now(&self) -> StdInstant {
        let _entered = self.0.enter();
        Instant::now().into_std()
    }
}

/// Builds a read control whose absolute cutoff `clock` reads.
pub(super) fn read_control(
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    clock: Arc<dyn Clock>,
) -> ReadControl {
    ReadControl {
        deadline: deadline.into_std(),
        clock,
        cancelled,
    }
}

/// The largest assembly window: evidence assembly keeps two of them.
const MAX_ASSEMBLY_WINDOW: Duration = Duration::from_millis(300);
/// The largest portion of the request deadline reserved for T032.
const MAX_T032_RESERVE: Duration = Duration::from_millis(50);
/// The later stages reserve a tenth of the request deadline when it is the
/// larger bound, otherwise their measured assembly windows are the floor.
const LATER_STAGE_RESERVE_DIVISOR: u32 = 10;

/// How long the retrieval routes may run. A deadline is a safety cap, not
/// a quality cutoff: by default a route runs until only the later stages'
/// time is left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StageWindow {
    /// Until the request's deadline less the time fusion, the rerank and
    /// evidence assembly keep, wherever in the request the route starts.
    #[default]
    Derived,
    /// At most this long from the route's start, once its setup ended,
    /// and never past the setup bound: for experiments and tests.
    Fixed(Duration),
}

/// The one accepted deadline and its retrieval/T032 phase cutoffs.
#[derive(Debug, Clone, Copy)]
pub(super) struct Deadlines {
    /// The full request deadline passed to T032.
    pub(super) expires: Instant,
    /// The cutoff of the routes that need no setup, such as lexical.
    pub(super) routes: Instant,
    /// The latest a route may end, its setup included: under
    /// [`StageWindow::Derived`], the time fusion and the rerank keep, a
    /// tenth of the deadline and at least one assembly window, before
    /// `setup`; under [`StageWindow::Fixed`], `setup`. A route's one-time
    /// setup, such as loading its model, that ends later leaves the route
    /// no time.
    pub(super) routes_end: Instant,
    /// The latest the reranker's one-time setup, such as loading its model,
    /// and the rerank may end. After it, the larger of two windows or a tenth
    /// of the deadline, plus the T032 reserve, pays for candidate loading,
    /// the permission recheck and evidence assembly.
    pub(super) setup: Instant,
    /// The final retrieval cutoff, before reserving time for T032.
    pub(super) work: Instant,
    /// The assembly window, a quarter of the deadline and at most 300 ms:
    /// evidence assembly keeps two, and the rerank at least one after
    /// enrichment.
    pub(super) window: Duration,
    /// A route's fixed window, when the configuration sets one.
    pub(super) fixed: Option<Duration>,
}

impl Deadlines {
    /// The cutoff of a route whose one-time setup ended at `ready`: its
    /// fixed window from then, when it has one, but never past the routes'
    /// end.
    pub(super) fn route_after(&self, ready: Instant) -> Instant {
        self.fixed.map_or(self.routes_end, |window| {
            (ready + window).min(self.routes_end)
        })
    }

    /// The latest optional reranker enrichment may read sources: one
    /// assembly window before `setup`, so the rerank keeps its time. Only
    /// under [`StageWindow::Derived`] does the later-stage reserve ensure it
    /// never precedes `routes_end`; fixed windows may end later.
    pub(super) fn enrichment(&self) -> Instant {
        self.setup - self.window
    }
}

/// Derives every phase cutoff once from the accepted request budget and
/// the configured route window.
pub(super) fn from_budget(
    started: Instant,
    budget: RequestBudget,
    stage_window: StageWindow,
) -> Deadlines {
    let duration = Duration::from_millis(u64::from(budget.deadline_ms));
    let expires = started + duration;
    let window = (duration / 4).min(MAX_ASSEMBLY_WINDOW);
    let reserve = (duration / 10).min(MAX_T032_RESERVE);
    let work = expires - reserve;
    let setup = work - (window * 2).max(duration / LATER_STAGE_RESERVE_DIVISOR);
    let (routes, routes_end, fixed) = match stage_window {
        StageWindow::Derived => {
            let routes_end = setup - (duration / LATER_STAGE_RESERVE_DIVISOR).max(window);
            (routes_end, routes_end, None)
        }
        StageWindow::Fixed(fixed) => {
            // A window past the deadline ends at `setup` all the same.
            let fixed = fixed.min(duration);
            ((started + fixed).min(setup), setup, Some(fixed))
        }
    };
    Deadlines {
        expires,
        routes,
        routes_end,
        setup,
        work,
        window,
        fixed,
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

/// Whether optional enrichment may still read under `control`: neither
/// cancelled nor past its cutoff.
pub(super) fn open(control: &ReadControl) -> bool {
    open_at(control, control.now())
}

/// Whether optional enrichment remains open at the supplied clock instant.
pub(super) fn open_at(control: &ReadControl, now: StdInstant) -> bool {
    !control.cancelled.load(Ordering::Relaxed) && now < control.deadline
}
