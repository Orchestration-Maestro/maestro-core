//! Deadline-controlled SQLite readers and scoped chunk-batch loading.

use super::{
    error::Error,
    types::{IDENTIFIER_PROFILE, ReadControl, SearchRead},
};
use crate::{
    chunk_set::{self, Chunk},
    scope::ScopeSet,
    store::{self, Database},
};
use rusqlite::{Connection, OptionalExtension as _, Transaction, TransactionBehavior, params};
use std::{sync::atomic::Ordering, time::Duration};

/// The maximum busy wait on any kernel reader.
const MAX_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// SQLite checks the cancellation callback after this many virtual operations.
const PROGRESS_OPERATIONS: i32 = 1000;

impl ReadControl {
    /// Refuses work after cancellation or the absolute deadline.
    pub(super) fn check(&self) -> Result<(), Error> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(Error::Cancelled)
        } else if self.now() >= self.deadline {
            Err(Error::TimedOut)
        } else {
            Ok(())
        }
    }
}

/// Opens a controlled reader with its busy wait bounded by remaining time.
pub(super) fn controlled_reader(
    database: &Database,
    control: &ReadControl,
) -> Result<Connection, Error> {
    control.check()?;
    let reader = database.reader()?;
    control.check()?;
    let remaining = control
        .deadline
        .checked_duration_since(control.now())
        .ok_or(Error::TimedOut)?;
    reader.busy_timeout(remaining.min(MAX_BUSY_TIMEOUT))?;
    let cancelled = control.cancelled.clone();
    let deadline = control.deadline;
    let clock = control.clock.clone();
    reader.progress_handler(
        PROGRESS_OPERATIONS,
        Some(move || cancelled.load(Ordering::Relaxed) || clock.now() >= deadline),
    )?;
    control.check()?;
    Ok(reader)
}

/// Starts a deferred read transaction and verifies the ready profile marker
/// for the exact scoped generation and chunk set.
pub(super) fn ready_projection(
    transaction: &Transaction<'_>,
    read: &SearchRead<'_>,
) -> Result<(), Error> {
    read.control.check()?;
    let projection = transaction
        .query_row(
            &format!(
                "SELECT generation_search.identifier_profile, generation_search.ready
                 FROM generation_search JOIN generations
                   ON generations.id = generation_search.generation_id
                 WHERE generations.id = ?1 AND generations.chunk_set_id = ?2
                   AND generations.collection_id = ?3 AND {}",
                ScopeSet::collection_condition("generations.collection_id", 4)
            ),
            params![
                read.generation.id,
                read.generation.chunk_set_id,
                read.generation.collection_id,
                read.scopes.parameter(),
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
        )
        .optional()
        .map_err(|error| classify(error, read.control))?
        .ok_or(Error::ProjectionMissing)?;
    if projection.0 != IDENTIFIER_PROFILE {
        return Err(Error::ProfileMismatch {
            expected: IDENTIFIER_PROFILE.to_owned(),
            found: projection.0,
        });
    }
    if !projection.1 {
        return Err(Error::ProjectionMissing);
    }
    read.control.check()
}

impl Database {
    /// Whether an eligible, in-scope chunk owner in the pinned set has the
    /// exact version filter.
    ///
    /// # Errors
    ///
    /// [`Error::Cancelled`] or [`Error::TimedOut`] when the controlled read
    /// stops; [`Error::Store`] on SQL failure.
    pub fn version_exists(&self, read: &SearchRead<'_>) -> Result<bool, Error> {
        read.control.check()?;
        let Some(version) = read.version else {
            return Ok(true);
        };
        let scope = ScopeSet::source_condition("documents.collection_id", "documents.source_id", 4);
        let sql = format!(
            "SELECT EXISTS(
               SELECT 1 FROM chunks
               JOIN revisions ON revisions.id = chunks.revision_id
               JOIN documents ON documents.id = revisions.document_id
               JOIN quality_dispositions
                 ON quality_dispositions.revision_id = revisions.id
               WHERE chunks.chunk_set_id = ?2
                 AND documents.collection_id = ?3 AND {scope}
                 AND CASE json_type(revisions.metadata_json, '$.version')
                   WHEN 'text' THEN json_extract(revisions.metadata_json, '$.version') END = ?5
                 AND revisions.status <> 'failed'
                 AND quality_dispositions.disposition IN ('accepted', 'accepted_with_warnings')
                 AND EXISTS (SELECT 1 FROM generations
                   WHERE generations.id = ?1 AND generations.chunk_set_id = ?2
                     AND generations.collection_id = ?3)
             )"
        );
        let mut connection = controlled_reader(self, read.control)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|error| classify(error, read.control))?;
        let exists = transaction
            .query_row(
                &sql,
                params![
                    read.generation.id,
                    read.generation.chunk_set_id,
                    read.generation.collection_id,
                    read.scopes.parameter(),
                    version,
                ],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| classify(error, read.control))?;
        read.control.check()?;
        transaction
            .commit()
            .map_err(|error| classify(error, read.control))?;
        read.control.check()?;
        Ok(exists)
    }

    /// Loads only the requested chunk IDs from the pinned generation, filtered
    /// by owner-source scope, collection, version and current eligibility.
    ///
    /// The IDs are JSON-bound through `json_each`; this reader does not require
    /// an identifier-search readiness marker.
    ///
    /// # Errors
    ///
    /// [`Error::Cancelled`] or [`Error::TimedOut`] when the controlled read
    /// stops; [`Error::Store`] on SQL or decoding failure.
    pub fn search_chunks(
        &self,
        read: &SearchRead<'_>,
        ids: &[String],
    ) -> Result<Vec<Chunk>, Error> {
        read.control.check()?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let encoded_ids = serde_json::to_string(ids)
            .map_err(|_| Error::InvalidInput("chunk IDs could not be encoded".to_owned()))?;
        let mut connection = controlled_reader(self, read.control)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|error| classify(error, read.control))?;
        let version = read.version;
        let sql = search_chunks_sql();
        let result: Result<Vec<Chunk>, Error> = (|| {
            read.control.check()?;
            let mut statement = transaction
                .prepare(&sql)
                .map_err(|error| classify(error, read.control))?;
            let mut rows = statement
                .query(params![
                    encoded_ids,
                    read.generation.chunk_set_id,
                    read.generation.collection_id,
                    read.scopes.parameter(),
                    version,
                ])
                .map_err(|error| classify(error, read.control))?;
            let mut chunks = Vec::new();
            while let Some(row) = rows.next().map_err(|error| classify(error, read.control))? {
                read.control.check()?;
                chunks.push(
                    chunk_set::chunk_row(row).map_err(|error| classify(error, read.control))?,
                );
            }
            read.control.check()?;
            Ok(chunks)
        })();
        let chunks = result?;
        read.control.check()?;
        transaction
            .commit()
            .map_err(|error| classify(error, read.control))?;
        read.control.check()?;
        Ok(chunks)
    }
}

/// Builds the scoped chunk-selection SQL shared with its query-plan test.
pub(super) fn search_chunks_sql() -> String {
    format!(
        "SELECT DISTINCT {} FROM json_each(?1) AS requested
         CROSS JOIN chunks
         JOIN revisions ON revisions.id = chunks.revision_id
         JOIN documents ON documents.id = revisions.document_id
         JOIN quality_dispositions ON quality_dispositions.revision_id = revisions.id
         WHERE requested.value = chunks.id
           AND chunks.chunk_set_id = ?2 AND documents.collection_id = ?3 AND {}
           AND (?5 IS NULL OR
             CASE json_type(revisions.metadata_json, '$.version')
               WHEN 'text' THEN json_extract(revisions.metadata_json, '$.version') END = ?5)
           AND revisions.status <> 'failed'
           AND quality_dispositions.disposition IN ('accepted', 'accepted_with_warnings')
         ORDER BY chunks.id",
        chunk_set::COLUMNS,
        ScopeSet::source_condition("documents.collection_id", "documents.source_id", 4)
    )
}

/// Maps an interrupted SQLite operation to cancellation or timeout when the
/// control says it stopped for one of those reasons.
pub(super) fn classify(error: rusqlite::Error, control: &ReadControl) -> Error {
    if control.cancelled.load(Ordering::Relaxed) {
        Error::Cancelled
    } else if control.now() >= control.deadline {
        Error::TimedOut
    } else {
        Error::Store(store::Error::Sqlite(error))
    }
}
