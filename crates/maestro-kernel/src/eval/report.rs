//! Reports as the kernel records them: the artifact of each, the record that
//! indexes and pins it, and the event that journals it.

use super::error::Error;
use crate::{
    artifact::Digest,
    journal::{self, NewEvent, event},
    scope::ScopeSet,
    store::{Database, artifacts},
};
use rusqlite::{OptionalExtension as _, Row, Transaction, params, types::Type};
use serde_json::json;
use std::error;
use ulid::Ulid;

/// The type of the event that journals the record of a report.
pub const RECORDED: &str = "maestro.eval.report.recorded.v1";

/// The media type of a report's artifact.
const MEDIA: &str = "application/json";

/// The columns [`report_row`] reads, in its order.
const COLUMNS: &str = "id, collection_id, generation_id, suite, digest, recorded_at";

/// A report to record: its JSON, `maestro-eval-report/1`, and what indexes
/// it, which the caller takes from the report itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewReport<'a> {
    /// The collection whose generation it evaluates.
    pub collection_id: &'a str,
    /// The generation it evaluates, one of that collection's.
    pub generation: i64,
    /// The suite it ran, such as `synthetic`.
    pub suite: &'a str,
    /// Its JSON, which the kernel stores as it is given.
    pub json: &'a [u8],
}

/// A report as the kernel records it: what indexes it, and the artifact of
/// its JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Its ID, a ULID, naming the millisecond it was recorded in.
    pub id: Ulid,
    /// The collection whose generation it evaluates.
    pub collection_id: String,
    /// The generation it evaluates.
    pub generation: i64,
    /// The suite it ran.
    pub suite: String,
    /// The artifact of its JSON.
    pub digest: Digest,
    /// When it was recorded: RFC 3339 in UTC, to the millisecond.
    pub recorded_at: String,
}

impl Database {
    /// Stores the JSON of `report` as an artifact, then records the report,
    /// pins the artifact and journals the record, in one write: on the
    /// stream of its collection, `collection/<id>`, in the collection's
    /// scope. Each call records a report of its own, even of the same JSON.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownGeneration`] when its collection records no such
    /// generation, and [`Error::Store`] when the artifact cannot be stored or
    /// the database cannot record the report or its event: nothing is
    /// recorded then, and the artifact, stored without a pin, is garbage.
    pub fn record_eval_report(&self, report: &NewReport<'_>) -> Result<Report, Error> {
        let digest = self.put(report.json, MEDIA)?;
        self.write(|transaction| {
            let generation = transaction
                .query_row(
                    "SELECT 1 FROM generations WHERE id = ?1 AND collection_id = ?2",
                    params![report.generation, report.collection_id],
                    |_| Ok(()),
                )
                .optional()?;
            if generation.is_none() {
                return Err(Error::UnknownGeneration {
                    collection: report.collection_id.to_owned(),
                    generation: report.generation,
                });
            }
            let recorded = transaction.query_row(
                &format!(
                    "INSERT INTO eval_reports (id, collection_id, generation_id, suite, digest)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     RETURNING {COLUMNS}"
                ),
                params![
                    Ulid::generate().to_string(),
                    report.collection_id,
                    report.generation,
                    report.suite,
                    digest.as_str(),
                ],
                report_row,
            )?;
            artifacts::pin(transaction, &digest)?;
            journal(transaction, &recorded)?;
            Ok(recorded)
        })
    }

    /// The report `id`, if it is recorded and `scopes` covers the scope of its
    /// collection.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn eval_report(&self, scopes: &ScopeSet, id: Ulid) -> Result<Option<Report>, Error> {
        let found = self
            .reader()?
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM eval_reports WHERE id = ?1 AND {}",
                    ScopeSet::collection_condition("eval_reports.collection_id", 2)
                ),
                params![id.to_string(), scopes.parameter()],
                report_row,
            )
            .optional()?;
        Ok(found)
    }

    /// Every report of the collection `collection_id`, in the order they
    /// were recorded, if `scopes` covers the scope of the collection; none
    /// otherwise.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn eval_reports(
        &self,
        scopes: &ScopeSet,
        collection_id: &str,
    ) -> Result<Vec<Report>, Error> {
        let reader = self.reader()?;
        // Reports are never deleted, so rowid order is record order.
        let mut statement = reader.prepare(&format!(
            "SELECT {COLUMNS} FROM eval_reports WHERE collection_id = ?1 AND {}
             ORDER BY rowid",
            ScopeSet::collection_condition("eval_reports.collection_id", 2)
        ))?;
        let reports = statement
            .query_map(params![collection_id, scopes.parameter()], report_row)?
            .collect::<Result<_, _>>()?;
        Ok(reports)
    }
}

/// Journals inside `transaction` that `report` was recorded: on the stream
/// of its collection, in the collection's scope.
fn journal(transaction: &Transaction<'_>, report: &Report) -> Result<(), Error> {
    let collection = &report.collection_id;
    let data = json!({
        "report": report.id.to_string(),
        "collection": collection,
        "generation": report.generation,
        "suite": report.suite,
    });
    event::record(
        transaction,
        &NewEvent {
            stream: &journal::stream(collection),
            r#type: RECORDED,
            subject: &format!("eval-report/{}", report.id),
            scope: &format!("workspace/default/collection/{collection}"),
            data: &data,
        },
    )?;
    Ok(())
}

/// The report of a row of [`COLUMNS`]. An ID that is not a ULID or a digest
/// that is not one is an error, never a guess.
fn report_row(row: &Row<'_>) -> rusqlite::Result<Report> {
    let id: String = row.get(0)?;
    let digest: String = row.get(4)?;
    Ok(Report {
        id: Ulid::from_string(&id).map_err(|invalid| unreadable(0, invalid))?,
        collection_id: row.get(1)?,
        generation: row.get(2)?,
        suite: row.get(3)?,
        digest: Digest::parse(&digest).map_err(|invalid| unreadable(4, invalid))?,
        recorded_at: row.get(5)?,
    })
}

/// The error of the text of column `index`, which `invalid` tells why it
/// cannot be read.
fn unreadable(index: usize, invalid: impl error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(invalid))
}
