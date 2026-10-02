//! A settable clock for the blocking stages' cutoff tests.

use maestro_kernel::retrieval::{Clock, ReadControl};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};

/// A clock reading the instant the test last set.
#[derive(Debug)]
pub(in crate::search) struct ManualClock(Mutex<State>);

/// The current instant and an optional deterministic mid-operation advance.
#[derive(Debug)]
struct State {
    now: Instant,
    advance: Option<(usize, Instant)>,
}

impl ManualClock {
    /// A clock reading `now` until the test sets another instant.
    pub(in crate::search) fn at(now: Instant) -> Arc<Self> {
        Arc::new(Self(Mutex::new(State { now, advance: None })))
    }

    /// Moves the clock to `now`.
    pub(in crate::search) fn set(&self, now: Instant) {
        self.0.lock().unwrap().now = now;
    }

    /// Advances after `reads` more observations, independently of real time.
    pub(in crate::search) fn advance_after_reads(&self, reads: usize, now: Instant) {
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

/// The last instant before `deadline`: work may still start.
pub(in crate::search) fn just_before(deadline: Instant) -> Instant {
    deadline.checked_sub(Duration::from_nanos(1)).unwrap()
}

/// A live control whose clock reads `now` against `deadline`.
pub(in crate::search) fn control_at(deadline: Instant, now: Instant) -> ReadControl {
    ReadControl {
        deadline,
        clock: ManualClock::at(now),
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}

/// A live control whose stopped clock excludes fixture setup from its budget.
pub(in crate::search) fn control() -> ReadControl {
    let now = Instant::now();
    control_at(now + Duration::from_secs(5), now)
}
