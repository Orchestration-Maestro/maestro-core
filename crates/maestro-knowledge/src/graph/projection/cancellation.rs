//! Explicit read cancellation without a timeout, polling interval or detached native handle.

use std::sync::{
    Arc, Condvar, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

/// Cloneable one-way cancellation token for a native read operation.
#[derive(Debug, Clone, Default)]
pub struct ProjectionCancellation {
    /// Atomic state and wake channel shared with the scoped native interrupt worker.
    pub(super) state: Arc<State>,
}

/// No connection or database can escape through this feature-independent token.
#[derive(Debug, Default)]
pub(super) struct State {
    /// Cancellation is permanent; tokens are not reset/reused after cancellation.
    pub(super) cancelled: AtomicBool,
    /// Serializes notification with entering a wait; no protected data can be poisoned.
    pub(super) gate: Mutex<()>,
    /// Wakes interrupt workers on cancellation or query completion.
    pub(super) changed: Condvar,
}

impl ProjectionCancellation {
    /// Create an uncancelled token; this sets no query budget or timeout.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Request cancellation; an active native read maps this to Connection interrupt.
    pub fn cancel(&self) {
        let _guard = self
            .state
            .gate
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.state.cancelled.store(true, Ordering::Release);
        self.state.changed.notify_all();
    }

    /// Whether the caller already requested cancellation.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }
}
