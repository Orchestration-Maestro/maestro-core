use super::super::deadline::{BlockingFailure, DeadlineElapsed, run_blocking, until};
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

#[test]
fn budgets_derive_the_capped_route_window_and_t032_reserve() {
    let started = Instant::now();
    let short = super::super::deadline::from_budget(
        started,
        RequestBudget {
            deadline_ms: 1,
            ..RequestBudget::default()
        },
    );
    assert_eq!(short.expires, started + Duration::from_millis(1));
    assert_eq!(short.routes, started + Duration::from_micros(250));
    assert_eq!(short.work, started + Duration::from_micros(900));

    let long = super::super::deadline::from_budget(
        started,
        RequestBudget {
            deadline_ms: 10_000,
            ..RequestBudget::default()
        },
    );
    assert_eq!(long.expires, started + Duration::from_secs(10));
    assert_eq!(long.routes, started + Duration::from_millis(300));
    assert_eq!(long.work, started + Duration::from_millis(9_950));
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
