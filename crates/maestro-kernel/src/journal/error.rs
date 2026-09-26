//! Why the journal refused an operation.

use crate::store;
use std::{error, fmt};

/// Why the journal refused an operation.
#[derive(Debug)]
pub enum Error {
    /// The ack is behind the cursor: a cursor moves forward only, and this
    /// one stays where it is.
    Backwards {
        /// The consumer whose cursor it is.
        consumer: String,
        /// The stream the cursor is on.
        stream: String,
        /// The position the ack gave.
        position: u64,
        /// The position the cursor is at.
        current: u64,
    },
    /// The ack is past the last event of its stream: the cursor would skip
    /// the events recorded later up to that position, and stays where it is.
    PastEnd {
        /// The consumer whose cursor it is.
        consumer: String,
        /// The stream the cursor is on.
        stream: String,
        /// The position the ack gave.
        position: u64,
        /// The sequence of the stream's last event, 0 when it has none.
        last: u64,
    },
    /// The kernel's database refused the operation; the message and the
    /// source are its own.
    Store(store::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backwards {
                consumer,
                stream,
                position,
                current,
            } => write!(
                formatter,
                "the cursor of {consumer} on {stream} is at {current}: an ack at {position} \
                 would move it backwards"
            ),
            Self::PastEnd {
                consumer,
                stream,
                position,
                last,
            } => write!(
                formatter,
                "{stream} ends at sequence {last}: the cursor of {consumer} cannot move to \
                 {position}"
            ),
            Self::Store(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(error) => error.source(),
            Self::Backwards { .. } | Self::PastEnd { .. } => None,
        }
    }
}

impl From<store::Error> for Error {
    fn from(error: store::Error) -> Self {
        Self::Store(error)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(store::Error::from(error))
    }
}
