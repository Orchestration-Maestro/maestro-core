//! The journal: every change the kernel makes, recorded as an event in one
//! append-only table of the kernel's database (docs/architecture/04 §3,
//! building block B2; plan D3).
//!
//! An event belongs to a stream, the events of one key such as a collection
//! or a job. Recording it gives it a ULID and the next sequence of its
//! stream, 1 for the first, taken in the transaction that inserts it, so
//! writers in any thread or process never skip or repeat a sequence; the
//! database gives it its time. The kernel's other tables record the event of
//! a change in the transaction that makes it, through `event::record` in the
//! crate, so the change and its event are committed or rolled back together.
//!
//! The table is the outbox events leave the kernel through: a consumer reads
//! a stream after the commit, never inside it, and remembers how far it has
//! read with a cursor, the sequence of the last event it acknowledged. An ack
//! moves a cursor forward only, and never past the stream's last event; the
//! cursors are rows of the database, so they survive a restart. Triggers
//! refuse to update, delete or replace an event, whoever writes.

mod cursor;
mod error;
pub(crate) mod event;
#[cfg(test)]
mod tests;

pub use error::Error;
pub use event::{Event, Filter, NewEvent};
