//! Why the kernel refused to submit, lease, move or read a job.

use super::state::JobState;
use crate::{journal, store};
use std::{error, fmt};
use ulid::Ulid;

/// Why the kernel refused to submit, lease, move or read a job.
#[derive(Debug)]
pub enum Error {
    /// No job of this ID is recorded.
    UnknownJob(Ulid),
    /// The move is not one the job's state allows; the job stays as it is.
    IllegalMove {
        /// The job's ID.
        job: Ulid,
        /// The state it is in.
        from: JobState,
        /// The state the move would have put it in.
        to: JobState,
    },
    /// A lease holds the job: only its holder moves the job, and another
    /// takes the lease over only once it has expired. The job stays as it
    /// is.
    Held {
        /// The job's ID.
        job: Ulid,
        /// Who holds its lease.
        holder: String,
        /// The lease's expiry, RFC 3339 in UTC: a time to come when a lease is
        /// refused, and a time already past, or not, when a cancellation is.
        expires: String,
    },
    /// Another job holds the resource, queued or running: the job is not
    /// submitted.
    ResourceHeld {
        /// The resource.
        resource: String,
        /// The job that holds it.
        job: Ulid,
    },
    /// The lease is no longer the job's: another holder took it over, or the
    /// job ended. Its holder writes nothing more to the job, so a stalled
    /// process that wakes up never writes over its successor.
    Lost {
        /// The job's ID.
        job: Ulid,
        /// Who held the lease.
        holder: String,
        /// The lease's number among the job's leases.
        number: u64,
    },
    /// A time the lease would record, its heartbeat or its expiry, falls
    /// before 1970 or after 9999, outside what the kernel records; nothing
    /// changes.
    Time,
    /// The journal refused; the message and the source are its own.
    Journal(journal::Error),
    /// The kernel's database refused; the message and the source are its own.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownJob(id) => write!(formatter, "no job {id} is recorded"),
            Self::IllegalMove { job, from, to } => write!(
                formatter,
                "job {job} cannot move from {from} to {to}: a job moves only from queued to \
                 running, then to succeeded, failed or cancelled, or from queued to cancelled"
            ),
            Self::Held {
                job,
                holder,
                expires,
            } => write!(
                formatter,
                "job {job} is leased to {holder} with an expiry of {expires}: only its holder \
                 moves the job, and another takes the lease over only from that time on"
            ),
            Self::ResourceHeld { resource, job } => write!(
                formatter,
                "resource {resource} is held by job {job}, which is queued or running: another \
                 job on it is refused until that one ends"
            ),
            Self::Lost {
                job,
                holder,
                number,
            } => write!(
                formatter,
                "lease {number} of {holder} on job {job} was taken over, or its job ended: it \
                 writes nothing more"
            ),
            Self::Time => formatter.write_str(
                "a lease is recorded from 1970 to the end of 9999: its time or its expiry \
                 falls outside",
            ),
            Self::Journal(error) => fmt::Display::fmt(error, formatter),
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Journal(error) => error.source(),
            Self::Store(error) => error.source(),
            Self::UnknownJob(_)
            | Self::IllegalMove { .. }
            | Self::Held { .. }
            | Self::ResourceHeld { .. }
            | Self::Lost { .. }
            | Self::Time => None,
        }
    }
}

impl From<journal::Error> for Error {
    fn from(error: journal::Error) -> Self {
        Self::Journal(error)
    }
}

impl From<store::Error> for Error {
    fn from(error: store::Error) -> Self {
        Self::Store(error)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}
