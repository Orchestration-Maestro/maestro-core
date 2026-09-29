use super::super::deadline::{
    BlockingFailure, DeadlineElapsed, Deadlines, StageWindow, from_budget, run_blocking, until,
};
use maestro_kernel::evidence::RequestBudget;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration as StdDuration, Instant as StdInstant},
};
use tokio::{
    sync::oneshot,
    time::{Duration, Instant, advance, sleep},
};

/// The cutoffs of a request that started at `started` with a deadline of
/// `deadline_ms` and the route window `stage_window`.
fn cutoffs(started: Instant, deadline_ms: u32, stage_window: StageWindow) -> Deadlines {
    from_budget(
        started,
        RequestBudget {
            deadline_ms,
            ..RequestBudget::default()
        },
        stage_window,
    )
}

#[test]
fn budgets_derive_the_route_end_the_setup_bound_and_the_t032_reserve() {
    let started = Instant::now();
    let short = cutoffs(started, 1, StageWindow::Derived);
    assert_eq!(short.expires, started + Duration::from_millis(1));
    assert_eq!(short.work, started + Duration::from_micros(900));
    assert_eq!(short.setup, started + Duration::from_micros(400));
    // Fusion and the rerank keep one assembly window, a quarter of the
    // deadline, over a tenth of it.
    assert_eq!(short.routes_end, started + Duration::from_micros(150));
    assert_eq!(short.routes, short.routes_end);

    let long = cutoffs(started, 10_000, StageWindow::Derived);
    assert_eq!(long.expires, started + Duration::from_secs(10));
    assert_eq!(long.work, started + Duration::from_millis(9_950));
    // Evidence assembly keeps its measured need at any deadline: the T032
    // reserve and two assembly windows, 650 ms; setup gets the rest.
    assert_eq!(long.setup, started + Duration::from_millis(9_350));
    // Fusion and the rerank keep a tenth of the deadline; the routes get
    // the rest.
    assert_eq!(long.routes_end, started + Duration::from_millis(8_350));
    assert_eq!(long.routes, long.routes_end);

    let cap = cutoffs(started, 30_000, StageWindow::Derived);
    assert_eq!(cap.work, started + Duration::from_millis(29_950));
    assert_eq!(cap.setup, started + Duration::from_millis(29_350));
    assert_eq!(cap.routes_end, started + Duration::from_millis(26_350));
    assert_eq!(cap.routes, cap.routes_end);
}

#[test]
fn optional_enrichment_ends_one_assembly_window_before_setup_after_the_routes() {
    let started = Instant::now();
    for (deadline_ms, enrichment) in [
        (1, Duration::from_micros(150)),
        (10_000, Duration::from_millis(9_050)),
        (30_000, Duration::from_millis(29_050)),
    ] {
        let derived = cutoffs(started, deadline_ms, StageWindow::Derived);
        assert_eq!(derived.enrichment(), started + enrichment);
        assert!(derived.routes_end <= derived.enrichment());
    }
}

#[test]
fn a_derived_route_runs_until_the_routes_end_wherever_it_starts() {
    let started = Instant::now();
    let derived = cutoffs(started, 1500, StageWindow::Derived);
    // Fusion and the rerank keep one assembly window, 300 ms, over a tenth
    // of the deadline.
    assert_eq!(derived.routes_end, started + Duration::from_millis(550));
    for ready in [0, 300, 549, 700] {
        assert_eq!(
            derived.route_after(started + Duration::from_millis(ready)),
            derived.routes_end
        );
    }
}

#[test]
fn a_fixed_route_window_starts_when_its_setup_ends_and_never_passes_the_setup_bound() {
    let started = Instant::now();
    let fixed = cutoffs(
        started,
        1500,
        StageWindow::Fixed(Duration::from_millis(300)),
    );
    assert_eq!(fixed.setup, started + Duration::from_millis(850));
    assert_eq!(fixed.routes_end, fixed.setup);
    assert_eq!(fixed.routes, started + Duration::from_millis(300));
    assert_eq!(
        fixed.route_after(started + Duration::from_millis(500)),
        started + Duration::from_millis(800)
    );
    // Past the setup bound starts evidence assembly's time: 650 ms, of
    // which it took 340-500 ms on a real collection.
    assert_eq!(
        fixed.route_after(started + Duration::from_millis(700)),
        started + Duration::from_millis(850)
    );
    let too_long = cutoffs(started, 1500, StageWindow::Fixed(Duration::from_secs(2)));
    assert_eq!(too_long.routes, too_long.setup);
}

#[tokio::test(start_paused = true)]
async fn one_cutoff_keeps_a_completed_route_and_times_out_a_pending_route() {
    let cutoff = Instant::now() + Duration::from_millis(30);
    let completed = until(cutoff, async {
        sleep(Duration::from_millis(10)).await;
        "complete"
    });
    let pending = until(cutoff, async {
        sleep(Duration::from_millis(100)).await;
        "late"
    });

    let (completed, pending) = tokio::join!(completed, pending);
    assert_eq!(completed, Ok("complete"));
    assert_eq!(pending, Err(DeadlineElapsed));
    assert_eq!(Instant::now(), cutoff);
}

#[tokio::test(start_paused = true)]
async fn an_expired_cutoff_does_not_poll_ready_work() {
    let started = Arc::new(AtomicBool::new(false));
    let worker_started = started.clone();
    let result = until(Instant::now(), async move {
        worker_started.store(true, Ordering::Relaxed);
    })
    .await;

    assert_eq!(result, Err(DeadlineElapsed));
    assert!(!started.load(Ordering::Relaxed));
}

#[tokio::test(start_paused = true)]
async fn timed_out_blocking_work_receives_cancellation_and_exits() {
    let cutoff = Instant::now() + Duration::from_millis(10);
    let (started, started_rx) = oneshot::channel();
    let (done, exited) = oneshot::channel::<bool>();
    let task = tokio::spawn(async move {
        run_blocking(cutoff, move |cancelled| {
            wait_for_cancellation(&cancelled, started, done);
        })
        .await
    });
    started_rx.await.unwrap();
    advance(Duration::from_millis(10)).await;

    assert_eq!(task.await.unwrap(), Err(BlockingFailure::TimedOut));
    assert!(
        exited.await.unwrap(),
        "blocking worker did not observe cancellation before its safety cutoff"
    );
}

fn wait_for_cancellation(
    cancelled: &AtomicBool,
    started: oneshot::Sender<()>,
    done: oneshot::Sender<bool>,
) {
    let _ = started.send(());
    let cutoff = StdInstant::now() + StdDuration::from_millis(100);
    while !cancelled.load(Ordering::Relaxed) && StdInstant::now() < cutoff {
        thread::sleep(StdDuration::from_millis(1));
    }
    let _ = done.send(cancelled.load(Ordering::Relaxed));
}
