//! Cursors: how far each consumer has read each stream, moved forward only.

use super::{error::Error, event::unsigned};
use crate::store::{self, Database};
use rusqlite::{Connection, OptionalExtension as _, params};

impl Database {
    /// The position of the cursor of `consumer` on `stream`: the sequence of
    /// the last event it acknowledged, 0 before its first ack.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn cursor(&self, consumer: &str, stream: &str) -> Result<u64, Error> {
        Ok(stored_position(&self.reader()?, consumer, stream)?)
    }

    /// Moves the cursor of `consumer` on `stream` to `position`, the
    /// sequence of the last event it processed, in a write of its own. An
    /// ack at the cursor's position writes nothing, so the cursor's
    /// `updated_at` is when its position last moved.
    ///
    /// # Errors
    ///
    /// [`Error::Backwards`] when `position` is behind the cursor,
    /// [`Error::PastEnd`] when the stream has no event at `position` yet, and
    /// [`Error::Store`] when the database cannot be written.
    pub fn ack(&self, consumer: &str, stream: &str, position: u64) -> Result<(), Error> {
        self.write(|transaction| {
            let last = last_sequence(transaction, stream)?;
            if position > last {
                return Err(Error::PastEnd {
                    consumer: consumer.to_owned(),
                    stream: stream.to_owned(),
                    position,
                    last,
                });
            }
            let current = stored_position(transaction, consumer, stream)?;
            if position < current {
                return Err(Error::Backwards {
                    consumer: consumer.to_owned(),
                    stream: stream.to_owned(),
                    position,
                    current,
                });
            }
            if position == current {
                return Ok(());
            }
            // At most the stream's last sequence, which SQLite's integers hold.
            let position = i64::try_from(position).unwrap_or(i64::MAX);
            transaction.execute(
                "INSERT INTO cursors (consumer, stream, position) VALUES (?1, ?2, ?3)
                 ON CONFLICT (consumer, stream) DO UPDATE
                 SET position = excluded.position, updated_at = excluded.updated_at",
                params![consumer, stream, position],
            )?;
            Ok(())
        })
    }
}

/// The position of the cursor of `consumer` on `stream` in `connection`, 0
/// when it has none.
fn stored_position(
    connection: &Connection,
    consumer: &str,
    stream: &str,
) -> Result<u64, store::Error> {
    let found = connection
        .query_row(
            "SELECT position FROM cursors WHERE consumer = ?1 AND stream = ?2",
            [consumer, stream],
            |row| unsigned(row, 0),
        )
        .optional()?;
    Ok(found.unwrap_or(0))
}

/// The sequence of the last event of `stream` in `connection`, 0 when it has
/// none.
fn last_sequence(connection: &Connection, stream: &str) -> Result<u64, store::Error> {
    let last = connection.query_row(
        "SELECT coalesce(max(sequence), 0) FROM events WHERE stream = ?1",
        [stream],
        |row| unsigned(row, 0),
    )?;
    Ok(last)
}
