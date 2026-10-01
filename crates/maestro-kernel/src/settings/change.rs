//! Recording a setting's change and reading a principal's changes back.

use crate::{
    journal::{self, Event, NewEvent, event},
    scope::WORKSPACE,
    store::Database,
};
use rusqlite::{Error as SqliteError, params, types::Type};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ulid::Ulid;

/// The type of the event of a setting's change.
pub const CHANGED: &str = event::SETTING_CHANGED;

/// One setting's change in one preferences file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingChange {
    /// Who changed it: the principal the command ran as.
    pub principal: String,
    /// The setting's dotted key.
    pub key: String,
    /// The value the file held before, `None` when it did not set the key.
    pub old: Option<Value>,
    /// The value the file holds after, `None` when the change unset the key.
    pub new: Option<Value>,
    /// Which file: `user` or `project`.
    pub layer: String,
    /// The file's path.
    pub file: String,
}

/// A change as the journal recorded it.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedChange {
    /// Its event's ID.
    pub id: Ulid,
    /// When it was recorded: RFC 3339 in UTC, from the database's clock.
    pub time: String,
    /// The change.
    pub change: SettingChange,
}

/// The stream of `principal`'s settings changes.
fn stream(principal: &str) -> String {
    format!("principal/{principal}/settings")
}

impl Database {
    /// Records `change` in the journal, on its principal's settings stream,
    /// in the workspace's scope.
    ///
    /// # Errors
    ///
    /// [`journal::Error::Store`] when the database cannot record it.
    pub fn record_setting_change(
        &self,
        change: &SettingChange,
    ) -> Result<RecordedChange, journal::Error> {
        let data = serde_json::to_value(change)
            .map_err(|error| SqliteError::ToSqlConversionFailure(Box::new(error)))?;
        let event = self.record(&NewEvent {
            stream: &stream(&change.principal),
            r#type: CHANGED,
            subject: &format!("setting/{}", change.key),
            scope: WORKSPACE,
            data: &data,
        })?;
        Ok(RecordedChange {
            id: event.id,
            time: event.time,
            change: change.clone(),
        })
    }

    /// Every settings change `principal` made, in the order recorded. It
    /// reads the principal's own stream, whatever its grants, and returns
    /// only the changes whose principal is `principal`.
    ///
    /// # Errors
    ///
    /// [`journal::Error::Store`] when the database cannot be read, or holds
    /// a change it cannot read back.
    pub fn setting_changes(&self, principal: &str) -> Result<Vec<RecordedChange>, journal::Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare(&format!(
            "SELECT {} FROM events WHERE stream = ?1 AND type = ?2 ORDER BY sequence",
            event::COLUMNS
        ))?;
        let events = statement
            .query_map(params![stream(principal), CHANGED], event::event_row)?
            .collect::<Result<Vec<Event>, _>>()?;
        let mut changes = Vec::with_capacity(events.len());
        for event in events {
            // Column 7 is the event's data.
            let change: SettingChange = serde_json::from_value(event.data).map_err(|error| {
                SqliteError::FromSqlConversionFailure(7, Type::Text, Box::new(error))
            })?;
            if change.principal == principal {
                changes.push(RecordedChange {
                    id: event.id,
                    time: event.time,
                    change,
                });
            }
        }
        Ok(changes)
    }
}
