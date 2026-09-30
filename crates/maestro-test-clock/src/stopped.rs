//! Holds Tokio time still while ordinary test work runs.

use std::{
    future::Future,
    pin::pin,
    sync::{mpsc, mpsc::RecvTimeoutError},
    time::Duration,
};
use tokio::{task, time};

/// Runs `work` on tokio's paused clock, held still until `release` ends;
/// from then on the clock moves only to the runtime's next timer, when every
/// task waits. Real time resumes when `work` ends.
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
pub async fn on_stopped_clock<T>(
    release: impl Future<Output = ()>,
    work: impl Future<Output = T>,
) -> T {
    time::pause();
    let (unhold, held) = mpsc::channel::<()>();
    let hold = task::spawn_blocking(move || held.recv_timeout(Duration::from_secs(10)));
    let mut unhold = Some(unhold);
    let mut release = pin!(release);
    let mut work = pin!(work);
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
