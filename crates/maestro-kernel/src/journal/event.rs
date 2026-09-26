//! Events: recorded with a new ID and the next sequence of their stream,
//! and read back in sequence order.

use super::error::Error;
use crate::store::{self, Database};
use rusqlite::{Row, Transaction, params, types::Type};
use serde_json::Value;
use std::error;
use ulid::Ulid;

/// The columns of an event, in the order [`event_row`] reads them.
const COLUMNS: &str = "id, stream, sequence, type, subject, scope, time, data";

/// An event as the journal records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Its ID, a ULID: unique in the journal, and naming the millisecond it
    /// was recorded in.
    pub id: Ulid,
    /// The stream it belongs to: the events of one key, a collection or a
    /// job among them, in the order they were recorded.
    pub stream: String,
    /// Its place in its stream: 1 for the first event, then one more for
    /// each, none skipped or repeated.
    pub sequence: u64,
    /// What happened, as a type name such as
    /// `maestro.knowledge.import.completed.v1`.
    pub r#type: String,
    /// What it happened to.
    pub subject: String,
    /// The path of the scope it belongs to.
    pub scope: String,
    /// When it was recorded: RFC 3339 in UTC, to the millisecond, from the
    /// database's clock.
    pub time: String,
    /// What it carries, as JSON.
    pub data: Value,
}

/// An event to record: the journal gives it its ID, its sequence and its
/// time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewEvent<'a> {
    /// The stream it belongs to.
    pub stream: &'a str,
    /// What happened.
    pub r#type: &'a str,
    /// What it happened to.
    pub subject: &'a str,
    /// The path of the scope it belongs to.
    pub scope: &'a str,
    /// What it carries.
    pub data: &'a Value,
}

/// Which events [`Database::events`] reads: those of one stream after a
/// position, of any type or of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Filter<'a> {
    /// The stream they belong to.
    pub stream: &'a str,
    /// The position they follow: only events of a greater sequence are read.
    /// 0 reads the whole stream, and a consumer's cursor what it has not
    /// acknowledged yet.
    pub after: u64,
    /// Their type, when only one is read.
    pub r#type: Option<&'a str>,
}

impl Database {
    /// Records `event` in a write of its own, with a new ID and the next
    /// sequence of its stream, and returns it as stored.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot record it.
    pub fn record(&self, event: &NewEvent<'_>) -> Result<Event, Error> {
        Ok(self.write(|transaction| record(transaction, event))?)
    }

    /// The events `filter` selects, in sequence order, as the last commit
    /// left them: a write in progress is not read.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read, or holds an event
    /// it cannot read back.
    pub fn events(&self, filter: &Filter<'_>) -> Result<Vec<Event>, Error> {
        // No event follows a position beyond what SQLite's integers hold.
        let after = i64::try_from(filter.after).unwrap_or(i64::MAX);
        let reader = self.reader()?;
        let mut statement = reader.prepare(&format!(
            "SELECT {COLUMNS} FROM events
             WHERE stream = ?1 AND sequence > ?2 AND (?3 IS NULL OR type = ?3)
             ORDER BY sequence"
        ))?;
        let events = statement
            .query_map(params![filter.stream, after, filter.r#type], event_row)?
            .collect::<Result<_, _>>()?;
        Ok(events)
    }
}

/// Records `event` inside `transaction`, a write that records the change
/// the event tells of, and returns it as stored.
///
/// # Errors
///
/// [`store::Error::Sqlite`] when the database cannot record it.
pub(crate) fn record(
    transaction: &Transaction<'_>,
    event: &NewEvent<'_>,
) -> Result<Event, store::Error> {
    let recorded = transaction.query_row(
        &format!(
            "INSERT INTO events (id, stream, sequence, type, subject, scope, data)
             SELECT ?1, ?2, coalesce(max(sequence), 0) + 1, ?3, ?4, ?5, ?6
             FROM events WHERE stream = ?2
             RETURNING {COLUMNS}"
        ),
        params![
            Ulid::generate().to_string(),
            event.stream,
            event.r#type,
            event.subject,
            event.scope,
            event.data.to_string(),
        ],
        event_row,
    )?;
    Ok(recorded)
}

/// The event of a row of [`COLUMNS`]. An ID that is not a ULID, data that is
/// not JSON serde reads, or a negative sequence is an error, never a guess.
fn event_row(row: &Row<'_>) -> rusqlite::Result<Event> {
    let id: String = row.get(0)?;
    let data: String = row.get(7)?;
    Ok(Event {
        id: Ulid::from_string(&id).map_err(|invalid| unreadable(0, invalid))?,
        stream: row.get(1)?,
        sequence: unsigned(row, 2)?,
        r#type: row.get(3)?,
        subject: row.get(4)?,
        scope: row.get(5)?,
        time: row.get(6)?,
        data: serde_json::from_str(&data).map_err(|invalid| unreadable(7, invalid))?,
    })
}

/// The error of the text of column `index`, which `invalid` tells why it
/// cannot be read.
fn unreadable(index: usize, invalid: impl error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(invalid))
}

/// The integer of column `index`, which the journal's tables keep at zero or
/// above.
pub(super) fn unsigned(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
