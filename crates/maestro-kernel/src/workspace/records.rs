//! Private user-local workspace answers replayed from the existing journal.
use crate::{
    journal::{self, event},
    scope::WORKSPACE,
    store::{self, Database},
};
use rusqlite::{Error as SqliteError, params, types::Type};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::{collections::BTreeMap, path::PathBuf};
use ulid::Ulid;

/// Private event kind; never delivered to scoped journal consumers.
pub const ANSWERED: &str = event::WORKSPACE_ANSWERED;
/// One user-local authority stream, independent of knowledge grants.
pub(super) const STREAM: &str = "principal/local/workspaces";

/// The trusted user confirmation channel, not text supplied by a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confirmation {
    /// An affirmative response on a terminal, defaulting to no.
    Terminal,
    /// An exact canonical absolute path repeated on the CLI.
    ConfirmPath,
}

/// An answer or digest-bound preferences-only receipt; none implicitly approve trust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    /// Explicit folder trust.
    Approved {
        /// User-only confirmation channel.
        confirmation: Confirmation,
    },
    /// Default-no or explicit user decline.
    Declined,
    /// Revocation of this root for subsequent controlled access.
    Removed,
    /// Separate approval for only the preferences file, not other workspace effects.
    Preferences {
        /// Exact file content digest, including its algorithm prefix.
        digest: String,
        /// Separate user-only confirmation channel.
        confirmation: Confirmation,
        /// Whether the held-handle file write completed.
        completed: bool,
    },
}

/// Typed persisted payload; field order is pinned independently of JSON map backends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceAnswer {
    /// Canonical absolute directory selected by the user.
    pub path: PathBuf,
    /// The user's answer or preferences-only receipt.
    pub answer: Answer,
}

/// Journal provenance attached to a replayed answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceRecord {
    /// Journal-generated approval receipt ID.
    pub id: String,
    /// Journal-generated UTC time.
    pub time: String,
    /// Canonical path and typed answer.
    pub change: WorkspaceAnswer,
}

impl Database {
    /// Record a user answer from the trusted CLI adapter, not discovered text.
    ///
    /// # Errors
    /// Returns serialization or database failures; relative paths are refused.
    pub fn record_workspace_answer(
        &self,
        change: &WorkspaceAnswer,
    ) -> Result<WorkspaceRecord, journal::Error> {
        if !change.path.is_absolute() {
            return Err(SqliteError::InvalidParameterName(
                "workspace path must be absolute".into(),
            )
            .into());
        }
        let bytes = serde_json::to_vec(change)
            .map_err(|error| SqliteError::ToSqlConversionFailure(Box::new(error)))?;
        let data = String::from_utf8(bytes)
            .map_err(|error| SqliteError::ToSqlConversionFailure(Box::new(error)))?;
        let id = Ulid::generate();
        let time = self.write::<_, store::Error>(|transaction| {
            Ok(transaction.query_row(
                "INSERT INTO events (id, stream, sequence, type, subject, scope, data)
                 SELECT ?1, ?2, coalesce(max(sequence), 0) + 1, ?3, ?4, ?5, ?6
                 FROM events WHERE stream = ?2 RETURNING time",
                params![
                    id.to_string(),
                    STREAM,
                    ANSWERED,
                    change.path.to_str(),
                    WORKSPACE,
                    data
                ],
                |row| row.get(0),
            )?)
        })?;
        Ok(WorkspaceRecord {
            id: id.to_string(),
            time,
            change: change.clone(),
        })
    }

    /// Read private authority directly, without knowledge grants or preference parsing.
    ///
    /// # Errors
    /// Refuses malformed payloads and database failures instead of inferring authority.
    pub fn workspace_answers(&self) -> Result<Vec<WorkspaceRecord>, journal::Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare(
            "SELECT id, time, data FROM events WHERE stream = ?1 AND type = ?2 ORDER BY sequence",
        )?;
        Ok(statement
            .query_map(params![STREAM, ANSWERED], |row| {
                let id: String = row.get(0)?;
                let data: String = row.get(2)?;
                let invalid = |error: Box<dyn Error + Send + Sync>| {
                    SqliteError::FromSqlConversionFailure(2, Type::Text, error)
                };
                Ok(WorkspaceRecord {
                    id: Ulid::from_string(&id)
                        .map_err(|error| invalid(Box::new(error)))?
                        .to_string(),
                    time: row.get(1)?,
                    change: serde_json::from_str(&data)
                        .map_err(|error| invalid(Box::new(error)))?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    /// Replay the last trust answer per canonical path; preferences receipts grant nothing.
    ///
    /// # Errors
    /// Returns the private journal read failure.
    pub fn trusted_workspaces(&self) -> Result<Vec<WorkspaceRecord>, journal::Error> {
        let mut roots = BTreeMap::new();
        for record in self.workspace_answers()? {
            match &record.change.answer {
                Answer::Approved { .. } => {
                    roots.insert(record.change.path.clone(), record);
                }
                Answer::Declined | Answer::Removed => {
                    roots.remove(&record.change.path);
                }
                Answer::Preferences { .. } => {}
            }
        }
        Ok(roots.into_values().collect())
    }
}
