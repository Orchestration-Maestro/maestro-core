//! A settable clock for the blocking stages' cutoff tests.

use maestro_kernel::retrieval::{Clock, ReadControl};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};

/// A clock reading the instant the test last set.
#[derive(Debug)]
pub(in crate::search) struct ManualClock(Mutex<Instant>);

impl ManualClock {
    /// A clock reading `now` until the test sets another instant.
    pub(in crate::search) fn at(now: Instant) -> Arc<Self> {
        Arc::new(Self(Mutex::new(now)))
    }

    /// Moves the clock to `now`.
    pub(in crate::search) fn set(&self, now: Instant) {
        *self.0.lock().unwrap() = now;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
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
