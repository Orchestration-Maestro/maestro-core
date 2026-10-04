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
    #[cfg(test)]
    let _liveness = tests::Liveness::new();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{panic::catch_unwind, process, sync::mpsc, time::Duration};

    /// Test-only wedge protection, not a native query deadline or latency assertion.
    /// Pi has no native-query test equivalent. Ten seconds leaves scheduler headroom.
    pub(super) struct Liveness {
        stop: mpsc::Sender<()>,
        worker: Option<thread::JoinHandle<()>>,
    }

    impl Liveness {
        pub(super) fn new() -> Self {
            let (stop, wait) = mpsc::channel();
            let worker = thread::spawn(move || watch(&wait));
            Self {
                stop,
                worker: Some(worker),
            }
        }
    }

    /// Fail the test process rather than leaving a lost scoped join waiting forever.
    fn watch(wait: &mpsc::Receiver<()>) {
        if wait.recv_timeout(Duration::from_secs(10)).is_ok() {
            return;
        }
        eprintln!("projection cancellation lost completion: 10 s liveness bound exceeded");
        // A panic cannot unwind a stuck scoped join. Abort this test process instead.
        process::abort();
    }

    impl Drop for Liveness {
        fn drop(&mut self) {
            self.stop.send(()).unwrap();
            self.worker.take().unwrap().join().unwrap();
        }
    }

    #[test]
    fn cancellation_wait_returns_on_either_completion_or_cancellation() {
        let _liveness = Liveness::new();
        for cancelled in [false, true] {
            let token = ProjectionCancellation::new();
            if cancelled {
                token.cancel();
            }
            wait(&token, &AtomicBool::new(!cancelled));
        }
    }

    #[test]
    fn cancellation_wait_cannot_return_before_a_wake_condition() {
        let _liveness = Liveness::new();
        let token = ProjectionCancellation::new();
        let done = AtomicBool::new(false);
        let (returned, result) = mpsc::channel();
        thread::scope(|scope| {
            // Holding the gate prevents a correct waiter from passing before cancellation.
            let gate = token.state.gate.lock().unwrap();
            scope.spawn(|| {
                wait(&token, &done);
                returned.send(()).unwrap();
            });
            let premature = result.recv_timeout(Duration::from_secs(1)).is_ok();
            drop(gate);
            token.cancel();
            assert!(
                !premature,
                "wait returned without cancellation or completion"
            );
            result
                .recv_timeout(Duration::from_secs(5))
                .expect("cancellation must wake the waiter");
        });
    }

    #[test]
    fn cancellation_completion_drop_sets_done_and_wakes_even_during_unwind() {
        let token = ProjectionCancellation::new();
        let done = AtomicBool::new(false);
        let result = catch_unwind(|| {
            let _completion = Completion {
                token: &token,
                done: &done,
            };
            panic!("decoding failed");
        });
        assert!(result.is_err());
        assert!(
            done.load(Ordering::Acquire),
            "unwinding must signal completion"
        );
    }
}

#[cfg(test)]
mod fence_tests {
    use super::*;
    use crate::graph::projection::engine::tests::Fixture;
    use std::cell::Cell;

    #[test]
    fn guard_cancel_run_pre_cancelled_skips_operation_and_post_decode_cancel_suppresses_result() {
        let fixture = Fixture::new();
        let database = fixture.writer();
        let connection = Connection::new(&database).unwrap();
        let cancelled = ProjectionCancellation::new();
        cancelled.cancel();
        let calls = Cell::new(0);
        let result = run(&connection, &cancelled, || {
            calls.set(calls.get() + 1);
            Ok(42)
        });
        assert_eq!(result, Err("projection read cancelled".into()));
        assert_eq!(calls.get(), 0, "pre-cancelled work must never decode");
        let token = ProjectionCancellation::new();
        let result = run(&connection, &token, || {
            token.cancel();
            Ok(42)
        });
        assert_eq!(result, Err("projection read cancelled".into()));
    }
}
