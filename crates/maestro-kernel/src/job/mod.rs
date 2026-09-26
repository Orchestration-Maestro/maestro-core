//! Jobs: long work, such as an import, a preparation or a publication, run
//! under a lease and resumed from its progress in the journal (building block
//! B4; plan D5).
//!
//! A caller submits a job with its kind, the frozen inputs it chose, the scope
//! it works on, its collection's, and the resource it holds, if any. The
//! job's ID is a ULID, and its idempotency key is the SHA-256 digest of its
//! kind, its scope and its inputs. A retried command submits the same kind,
//! scope and inputs, so it finds the job of its key while that job is queued
//! or running, and once it succeeded. After a failure or a cancellation the
//! key starts a new job, the next attempt; whether that attempt resumes from
//! the last one's progress is the caller's choice.
//!
//! A job is read, as every record of the kernel is, through the caller's
//! `ScopeSet`: [`Database::job`](crate::store::Database::job) and
//! [`Database::last_progress`](crate::store::Database::last_progress) see
//! only the jobs whose scope the set covers, and one it does not cover reads
//! as unknown. The calls that take, renew and end a lease name a job by the
//! ID or the lease a submission or a reader gave.
//!
//! A resource, such as the publication of a collection, is a name the caller
//! chooses: at most one job queued or running holds each, so a second job on
//! it is refused, naming the job that holds it, until that one ends. The key
//! comes first, so a retried command still finds its own job. A publication
//! of a collection thus has its own key, from what it publishes, and the
//! collection's publication as its resource.
//!
//! A job moves only forward: from `queued` to `running`, then to `succeeded`,
//! `failed` or `cancelled`, or from `queued` to `cancelled`. The three
//! outcomes are final, each with the JSON its holder or its canceller gave.
//! A running job holds a lease: its holder, its number, its heartbeat and
//! its expiry. A lease that is held and has not expired cannot be taken, and
//! the refusal names the holder and the expiry, so no two processes work on
//! one job. Once the lease has expired, another holder takes it over with the
//! next number, and the job keeps running. The job knows its lease by that
//! number: a heartbeat, a step or an outcome under a lease that was taken over
//! is refused and writes nothing, so a stalled process that wakes up never
//! writes over its successor. A holder whose lease expired, and that no one
//! took over, still renews it. The table refuses the same, whoever writes: a
//! job that ended never changes, a state moves only forward, and a lease
//! number never decreases.
//!
//! Time comes from the caller's clock: each call that takes or renews a
//! lease is given `now`, and the kernel compares an expiry with that time
//! and with no other clock. SQLite writes the times as the kernel's other
//! tables write theirs: RFC 3339 in UTC, to the millisecond.
//!
//! The journal holds a job's life on its stream `job/<id>`, each event
//! recorded in the write that makes the change it tells of, so both commit or
//! neither: its creation with its inputs, and the attempt before it if any;
//! its first lease, and each takeover with the lease it took over; each of its
//! steps, in the write that renews its lease; and its end with its outcome.
//! A heartbeat renews the lease only, and a retried command changes nothing:
//! neither records an event. A process that resumes a job, after a takeover
//! or in a new attempt, reads its last step and continues after it.

mod error;
mod events;
mod lease;
mod outcome;
mod progress;
mod record;
mod state;
#[cfg(test)]
mod tests;

pub use error::Error;
pub use events::{CANCELLED, CREATED, FAILED, PROGRESSED, SUCCEEDED, TAKEN, TAKEN_OVER, stream};
pub use record::{Job, Lease, NewJob};
pub use state::JobState;
