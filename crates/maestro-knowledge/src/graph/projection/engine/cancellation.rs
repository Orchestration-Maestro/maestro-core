//! Scoped native interrupt relay; no native reference survives a read.
use crate::graph::projection::cancellation::ProjectionCancellation;
use lbug::Connection;
use std::{
    sync::{
        PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

/// Join the interrupt worker before dropping its borrowed connection/database.
pub(super) fn run<T>(
    connection: &Connection<'_>,
    token: &ProjectionCancellation,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if token.is_cancelled() {
        return Err("projection read cancelled".into());
    }
    let done = AtomicBool::new(false);
    thread::scope(|scope| {
        let interrupt = scope.spawn(|| {
            wait(token, &done);
            // Native query startup resets its interrupt flag. Repeat after cancellation
            // until completion so a cancel racing startup cannot be lost.
            while !done.load(Ordering::Acquire) {
                connection.interrupt().map_err(|error| error.to_string())?;
                thread::yield_now();
            }
            Ok::<_, String>(())
        });
        let completion = Completion { token, done: &done };
        let result = operation();
        drop(completion);
        interrupt
            .join()
            .map_err(|_| "native cancellation worker failed")??;
        if token.is_cancelled() {
            return Err("projection read cancelled".into());
        }
        result
    })
}

/// Wakes and joins the interrupt relay even if native decoding panics.
struct Completion<'a> {
    /// Token's wake channel.
    token: &'a ProjectionCancellation,
    /// Per-operation completion flag, never shared with another query.
    done: &'a AtomicBool,
}
impl Drop for Completion<'_> {
    fn drop(&mut self) {
        finished(self.token, self.done);
    }
}

/// Sleep on the token wake channel until cancellation or this query's completion.
fn wait(token: &ProjectionCancellation, done: &AtomicBool) {
    let mut guard = token
        .state
        .gate
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    while !token.is_cancelled() && !done.load(Ordering::Acquire) {
        guard = token
            .state
            .changed
            .wait(guard)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// Wake the interrupt worker on normal completion and unwinding alike.
fn finished(token: &ProjectionCancellation, done: &AtomicBool) {
    let _guard = token
        .state
        .gate
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    done.store(true, Ordering::Release);
    token.state.changed.notify_all();
}
