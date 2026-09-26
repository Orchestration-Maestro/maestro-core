//! The lease of a job this process works on in the foreground (plan D5,
//! T016), held only by `Holder::run`: each step its work journals renews
//! it, and between steps a heartbeat thread does, until the work ends and
//! the job with it. A lease taken over by another process refuses every
//! later write of this one.

use maestro_kernel::{
    job::{self, Job, JobState, Lease},
    store::Database,
};
use serde_json::Value;
use std::{
    iter, process,
    sync::{
        Mutex, MutexGuard, PoisonError,
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::{Duration, SystemTime},
};

/// How long a lease lasts, and how often its holder renews it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Timing {
    /// How long a lease lasts from its last renewal.
    pub(super) term: Duration,
    /// How long the heartbeat thread waits between two renewals.
    pub(super) beat: Duration,
}

/// The command line's leases: 60 s, renewed every 20 s, so that two missed
/// heartbeats still leave the job to this process.
pub(super) const TIMING: Timing = Timing {
    term: Duration::from_secs(60),
    beat: Duration::from_secs(20),
};

/// The name this process's leases are held under: the command line and its
/// process ID.
pub(super) fn holder() -> String {
    format!("maestro-cli/{}", process::id())
}

/// A job's lease that this process holds.
#[derive(Debug)]
pub(super) struct Holder<'a> {
    /// The kernel whose job it is.
    database: &'a Database,
    /// The lease, as its last renewal left it.
    lease: Mutex<Lease>,
    /// How long it lasts, and how often it is renewed.
    timing: Timing,
}

impl<'a> Holder<'a> {
    /// Holds `lease`, a lease of a job of `database`, while `work` runs,
    /// then ends the job in the state `work` returns, with its outcome, and
    /// returns the job as it ended. A thread of its own renews the lease every
    /// beat of `timing` until `work` has returned, and each step `work`
    /// journals through the holder renews it too. Only this gives a holder,
    /// so no work runs under a lease without its heartbeats.
    ///
    /// # Errors
    ///
    /// The kernel's refusal to end the job, [`job::Error::Lost`] among them
    /// once another process took the lease over.
    pub(super) fn run(
        database: &'a Database,
        lease: Lease,
        timing: Timing,
        work: impl FnOnce(&Self) -> (JobState, Value),
    ) -> Result<Job, job::Error> {
        let holder = Self {
            database,
            lease: Mutex::new(lease),
            timing,
        };
        let (state, outcome) = thread::scope(|scope| {
            // Dropped once the work returns, or as a panic of the work
            // unwinds, which ends the heartbeats either way: the scope waits
            // for their thread, which never outlives the work.
            let (stop, stopped) = mpsc::channel::<()>();
            scope.spawn(|| holder.beat_at(ticks(stopped, timing.beat)));
            let ended = work(&holder);
            drop(stop);
            ended
        });
        let lease = holder
            .lease
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        database.complete_job(&lease, state, &outcome)
    }

    /// Journals `data` as the job's next step, renewing the lease in the same
    /// write.
    ///
    /// # Errors
    ///
    /// The kernel's refusal, [`job::Error::Lost`] among them once another
    /// process took the lease over: nothing is written then.
    pub(super) fn step(&self, data: &Value) -> Result<(), job::Error> {
        let mut lease = self.lock();
        self.database
            .progress(&mut lease, SystemTime::now(), self.timing.term, data)
            .map(drop)
    }

    /// Renews the lease at each of `ticks`, until they end or the lease is
    /// lost; a renewal that fails otherwise is tried again at the next.
    pub(super) fn beat_at(&self, ticks: impl Iterator<Item = ()>) {
        for () in ticks {
            let mut lease = self.lock();
            let renewed = self
                .database
                .heartbeat(&mut lease, SystemTime::now(), self.timing.term);
            if let Err(job::Error::Lost { .. }) = renewed {
                break;
            }
        }
    }

    /// The lease, for one renewal: a thread that panicked holding it left it
    /// as the kernel last returned it.
    fn lock(&self) -> MutexGuard<'_, Lease> {
        self.lease.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A tick after each `beat` that passes before `stopped` hears its sender
/// go.
pub(super) fn ticks(stopped: Receiver<()>, beat: Duration) -> impl Iterator<Item = ()> {
    iter::from_fn(move || match stopped.recv_timeout(beat) {
        Err(RecvTimeoutError::Timeout) => Some(()),
        Ok(()) | Err(RecvTimeoutError::Disconnected) => None,
    })
}
