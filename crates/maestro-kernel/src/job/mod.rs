//! Jobs: long work, such as an import, a preparation or a publication, run
//! under a lease and resumed from its progress in the journal (building block
//! B4; plan D5).
//!
//! A caller submits a job with its kind, the frozen inputs it chose and the
//! path of the scope it works on. The job's ID is a ULID, and its idempotency
//! key is the SHA-256 digest of its kind and its inputs. A retried command
//! submits the same kind and inputs, so it finds the job of its key while
//! that job is queued or running, and once it succeeded. After a failure or
//! a cancellation the key starts a new job, the next attempt; whether that
//! attempt resumes from the last one's progress is the caller's choice.
//!
//! A job moves only forward: from `queued` to `running`, then to `succeeded`,
//! `failed` or `cancelled`, or from `queued` to `cancelled`. The three
//! outcomes are final, each with the JSON its holder or its canceller gave.
//! A running job holds a lease: its holder, its number, its heartbeat and
//! its expiry. A lease that is held and has not expired cannot be taken, and
//! the refusal names the holder and the expiry, so no two processes work on
//! one job, nor on one key: that is how a second publication of a collection
//! is refused. Once the lease has expired, another holder takes it over with
//! the next number, and the job keeps running. The job knows its lease by
//! that number: a heartbeat, a step or an outcome under a lease that was
//! taken over is refused and writes nothing, so a stalled process that wakes
//! up never writes over its successor. A holder whose lease expired, and
//! that no one took over, still renews it.
//!
//! Time comes from the caller's clock: each call that takes or renews a
//! lease is given `now`, and the kernel compares an expiry with that time
//! and with no other clock. SQLite writes the times as the kernel's other
//! tables write theirs: RFC 3339 in UTC, to the millisecond.
//!
//! Each step of a job goes to the journal, on the job's stream `job/<id>`,
//! in the write that renews its lease: one write per step, which commits
//! both or neither. A heartbeat renews the lease only, and records no event.
//! A process that resumes a job, after a takeover or in a new attempt, reads
//! its last step and continues after it.

mod error;
mod lease;
mod outcome;
mod progress;
mod record;
mod state;
#[cfg(test)]
mod tests;

pub use error::Error;
pub use progress::{PROGRESSED, stream};
pub use record::{Job, Lease, NewJob};
pub use state::JobState;
