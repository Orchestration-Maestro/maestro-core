use super::super::deadline::{
    BlockingFailure, DeadlineElapsed, Deadlines, RuntimeClock, StageWindow, from_budget, open_at,
    run_blocking, until,
};
use maestro_kernel::retrieval::SystemClock;
use maestro_kernel::{evidence::RequestBudget, retrieval::ReadControl};
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

#[test]
fn enrichment_cutoff_is_exclusive_at_the_deadline() {
    let now = StdInstant::now();
    let deadline = now + StdDuration::from_secs(1);
    let control = ReadControl {
        deadline,
        clock: Arc::new(SystemClock),
        cancelled: Arc::new(AtomicBool::new(false)),
    };

    assert!(open_at(&control, now));
    assert!(!open_at(&control, deadline));
    assert!(!open_at(&control, deadline + StdDuration::from_nanos(1)));
}

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
fn one_millisecond_budget_keeps_its_minimum_cutoffs() {
    let started = Instant::now();
    let short = cutoffs(started, 1, StageWindow::Derived);
    assert_eq!(short.expires, started + Duration::from_millis(1));
    assert_eq!(short.work, started + Duration::from_micros(900));
    assert_eq!(short.setup, started + Duration::from_micros(400));
    // One assembly window, a quarter of the deadline, exceeds a tenth here.
    assert_eq!(short.routes_end, started + Duration::from_micros(150));
    assert_eq!(short.routes, short.routes_end);
}

#[test]
fn one_point_five_second_budget_keeps_the_t032_reserve() {
    let started = Instant::now();
    let minimum = cutoffs(started, 1500, StageWindow::Derived);
    assert_eq!(minimum.setup, started + Duration::from_millis(850));
    assert_eq!(minimum.expires - minimum.setup, Duration::from_millis(650));
    assert_eq!(minimum.routes_end, started + Duration::from_millis(550));
}

#[test]
fn ten_and_thirty_second_budgets_scale_the_reserve() {
    let started = Instant::now();
    let ten_seconds = cutoffs(started, 10_000, StageWindow::Derived);
    assert_eq!(ten_seconds.expires, started + Duration::from_secs(10));
    assert_eq!(ten_seconds.work, started + Duration::from_millis(9_950));
    // Assembly gets max(two windows, a tenth of the deadline), plus T032.
    assert_eq!(ten_seconds.setup, started + Duration::from_millis(8_950));
    assert_eq!(
        ten_seconds.routes_end,
        started + Duration::from_millis(7_950)
    );
    assert_eq!(ten_seconds.routes, ten_seconds.routes_end);

    let thirty_seconds = cutoffs(started, 30_000, StageWindow::Derived);
    assert_eq!(thirty_seconds.work, started + Duration::from_millis(29_950));
    assert_eq!(
        thirty_seconds.setup,
        started + Duration::from_millis(26_950)
    );
    assert_eq!(
        thirty_seconds.routes_end,
        started + Duration::from_millis(23_950)
    );
    assert_eq!(thirty_seconds.routes, thirty_seconds.routes_end);
}

#[test]
fn optional_enrichment_ends_one_assembly_window_before_setup_after_the_routes() {
    let started = Instant::now();
    for (deadline_ms, enrichment) in [
        (1, Duration::from_micros(150)),
        (10_000, Duration::from_millis(8_650)),
        (30_000, Duration::from_millis(26_650)),
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

#[test]
fn a_fixed_route_window_past_the_deadline_is_the_deadline() {
    let started = Instant::now();
    let unbounded = cutoffs(started, 30_000, StageWindow::Fixed(Duration::MAX));
    assert_eq!(unbounded.fixed, Some(Duration::from_secs(30)));
    assert_eq!(unbounded.routes, unbounded.setup);
    assert_eq!(unbounded.route_after(unbounded.setup), unbounded.routes_end);
}

#[tokio::test]
async fn runtime_clock_tracks_system_clock_without_paused_time() {
    use maestro_kernel::retrieval::{Clock, SystemClock};

    let runtime_clock = RuntimeClock::current();
    let runtime_now = runtime_clock.now();
    let system_now = SystemClock.now();
    let difference = if runtime_now >= system_now {
        runtime_now.duration_since(system_now)
    } else {
        system_now.duration_since(runtime_now)
    };
    assert!(difference < StdDuration::from_secs(1));
}

#[tokio::test(start_paused = true)]
async fn blocking_clock_control_ignores_a_real_stall_but_observes_expiry() {
    let cutoff = Instant::now() + Duration::from_millis(10);
    let control = ReadControl {
        deadline: cutoff.into_std(),
        clock: Arc::new(RuntimeClock::current()),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let before = control.now();
    thread::sleep(StdDuration::from_millis(25));
    assert!(control.now() < control.deadline);

    advance(Duration::from_millis(10)).await;
    assert!(control.now() >= control.deadline);
    assert!(control.now() > before);
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
