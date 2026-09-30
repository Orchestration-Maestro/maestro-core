//! A settable clock shared by controlled-read tests.

use crate::retrieval::{Clock, ReadControl};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};

/// A clock reading the instant the test last set.
#[derive(Debug)]
pub(super) struct ManualClock(Mutex<State>);

/// The current instant and an optional deterministic mid-operation advance.
#[derive(Debug)]
struct State {
    now: Instant,
    advance: Option<(usize, Instant)>,
}

impl ManualClock {
    /// A clock reading `now` until the test sets another instant.
    pub(super) fn at(now: Instant) -> Arc<Self> {
        Arc::new(Self(Mutex::new(State { now, advance: None })))
    }

    /// Moves the clock to `now`.
    pub(super) fn set(&self, now: Instant) {
        self.0.lock().unwrap().now = now;
    }

    /// Advances after `reads` more observations, independently of real time.
    pub(super) fn advance_after_reads(&self, reads: usize, now: Instant) {
        self.0.lock().unwrap().advance = Some((reads, now));
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        let mut state = self.0.lock().unwrap();
        if let Some((reads, now)) = state.advance {
            if reads == 0 {
                state.now = now;
                state.advance = None;
            } else {
                state.advance = Some((reads - 1, now));
            }
        }
        state.now
    }
}

/// A live control that cannot expire while its test prepares fixtures.
pub(in crate::retrieval) fn control() -> ReadControl {
    let now = Instant::now();
    ReadControl {
        deadline: now + Duration::from_secs(5),
        clock: ManualClock::at(now),
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}
