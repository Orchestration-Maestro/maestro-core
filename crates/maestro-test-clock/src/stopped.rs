//! Holds Tokio time still while ordinary test work runs.

use std::{
    future::Future,
    pin::pin,
    sync::{mpsc, mpsc::RecvTimeoutError},
    time::Duration,
};
use tokio::{task, time};

/// Calls `work` only after pausing tokio's clock, then runs its future with
/// the clock held still until `release` ends;
/// from then on the clock moves only to the runtime's next timer, when every
/// task waits. Real time resumes when `work` ends. Construct requests,
/// deadlines and clocks inside `work`; build clock-independent fixtures
/// before calling this helper.
///
/// Paused alone, tokio advances the clock to the next timer whenever the
/// runtime waits, and a loopback gRPC reply in flight is such a wait: its
/// readiness does not count as a wake, so a route's deadline could pass at
/// once. Tokio does not auto-advance while a blocking task runs ("Preventing
/// auto-advance" in the documentation of `tokio::time::pause`), so one
/// blocking task waiting for the release holds the clock still, however
/// loaded the host.
///
/// # Panics
///
/// When called outside a current-thread Tokio runtime, while its time is
/// already paused, or if the holder fails or work does not release it within
/// the existing ten-second hang guard.
#[expect(
    clippy::unwrap_used,
    reason = "a failed holder task must fail the calling test"
)]
pub async fn on_stopped_clock<T, F: Future<Output = T>>(
    release: impl Future<Output = ()>,
    work: impl FnOnce() -> F,
) -> T {
    time::pause();
    let (unhold, held) = mpsc::channel::<()>();
    let hold = task::spawn_blocking(move || held.recv_timeout(Duration::from_secs(10)));
    let mut unhold = Some(unhold);
    let mut release = pin!(release);
    let mut work = pin!(work());
    let output = loop {
        tokio::select! {
            output = &mut work => break output,
            () = &mut release, if unhold.is_some() => unhold = None,
        }
    };
    drop(unhold);
    assert_eq!(
        hold.await.unwrap(),
        Err(RecvTimeoutError::Disconnected),
        "clock held 10 s without a release"
    );
    time::resume();
    output
}

#[cfg(test)]
mod tests {
    use super::on_stopped_clock;
    use std::{future, thread, time::Duration};
    use tokio::time::Instant;

    #[tokio::test]
    async fn work_is_constructed_on_the_stopped_clock() {
        let work = || {
            let before = Instant::now();
            thread::sleep(Duration::from_millis(20));
            assert_eq!(Instant::now(), before);
            future::ready(42)
        };
        assert_eq!(on_stopped_clock(future::pending(), work).await, 42);
    }
}
