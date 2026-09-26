//! Revisions: one exact version of a document's bytes and metadata, recorded
//! once with the two artifacts it pins and never changed afterwards, but for
//! its status's one move, to failed.

use super::{collection::json, error::Error};
use crate::{
    artifact::Digest,
    store::{Database, artifacts},
};
use rusqlite::{Connection, OptionalExtension as _, Row, Transaction, params, types::Type};
use serde_json::{Map, Value};

/// The columns [`revision_row`] reads, in its order.
const COLUMNS: &str = "revisions.id, document_id, original_digest, canonical_digest, status, \
                       captured_at, metadata_json";

/// A revision: one exact version of a document's bytes and metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    /// Its id, canonicalization's recipe over the document, its bytes and its
    /// metadata (01 §2.1).
    pub id: String,
    /// The document it is a version of.
    pub document_id: String,
    /// The artifact holding its original Markdown.
    pub original_digest: Digest,
    /// The artifact holding its canonical document.
    pub canonical_digest: Digest,
    /// Canonicalization's verdict on it.
    pub status: RevisionStatus,
    /// When its bytes were captured, as its source states it, if it does.
    pub captured_at: Option<String>,
    /// Its metadata, as the import read it.
    pub metadata: Map<String, Value>,
}

/// Canonicalization's verdict on a revision (01 §5). It moves only to
/// [`RevisionStatus::Failed`], which it never leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionStatus {
    /// Every check passed: eligible.
    Valid,
    /// Recoverable issues remain explicit: eligible.
    ValidWithWarnings,
    /// A blocking issue: inspectable, never eligible.
    Failed,
}

impl RevisionStatus {
    /// Every status.
    const ALL: [Self; 3] = [Self::Valid, Self::ValidWithWarnings, Self::Failed];

    /// Its name, as the `status` column holds it.
    fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::ValidWithWarnings => "valid_with_warnings",
            Self::Failed => "failed",
        }
    }
}

/// What recording a revision did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    /// The revision is new: it is recorded and its two artifacts pinned.
    New,
    /// The revision was recorded before with the same content: nothing was
    /// written.
    Unchanged,
}

impl Database {
    /// Records `revision` and pins its original and canonical artifacts, in
    /// one transaction; a revision recorded before with the same content,
    /// whatever its status, is left as it is.
    ///
    /// # Errors
    ///
    /// [`Error::RevisionConflict`] when its id is recorded with other content,
    /// and [`Error::Store`] when an artifact or its document is not recorded
    /// or the database cannot record it: nothing is recorded then.
    pub fn record_revision(&self, revision: &Revision) -> Result<Recorded, Error> {
        self.write(|transaction| match find(transaction, &revision.id)? {
            Some(recorded) => record_again(recorded, revision),
            None => insert(transaction, revision),
        })
    }

    /// The revision `id`, if it is recorded, whatever its status.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn revision(&self, id: &str) -> Result<Option<Revision>, Error> {
        Ok(find(&self.reader()?, id)?)
    }

    /// Every revision of the collection `collection_id` that is not failed,
    /// in the order they were recorded.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn eligible_revisions(&self, collection_id: &str) -> Result<Vec<Revision>, Error> {
        let reader = self.reader()?;
        // Each insert takes a rowid above every other, so rowid order is
        // record order, whatever the clock did in between.
        let mut statement = reader.prepare(&format!(
            "SELECT {COLUMNS} FROM revisions JOIN documents ON documents.id = revisions.document_id
             WHERE documents.collection_id = ?1 AND status <> 'failed'
             ORDER BY revisions.rowid"
        ))?;
        let revisions = statement
            .query_map([collection_id], revision_row)?
            .collect::<Result<_, _>>()?;
        Ok(revisions)
    }
}

/// What recording `given` again does when its id is recorded as `recorded`:
/// nothing, if they differ in their status alone, which may have moved since.
fn record_again(recorded: Revision, given: &Revision) -> Result<Recorded, Error> {
    let recorded = Revision {
        status: given.status,
        ..recorded
    };
    if recorded == *given {
        Ok(Recorded::Unchanged)
    } else {
        Err(Error::RevisionConflict(given.id.clone()))
    }
}

/// Records the new `revision` inside `transaction`, and pins its two
/// artifacts there.
fn insert(transaction: &Transaction<'_>, revision: &Revision) -> Result<Recorded, Error> {
    transaction.execute(
        "INSERT INTO revisions (id, document_id, original_digest, canonical_digest, status,
           captured_at, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            revision.id,
            revision.document_id,
            revision.original_digest.as_str(),
            revision.canonical_digest.as_str(),
            revision.status.as_str(),
            revision.captured_at,
            Value::Object(revision.metadata.clone()).to_string(),
        ],
    )?;
    artifacts::pin(transaction, &revision.original_digest)?;
    artifacts::pin(transaction, &revision.canonical_digest)?;
    Ok(Recorded::New)
}

/// The revision `id` that `connection` records, if any.
fn find(connection: &Connection, id: &str) -> rusqlite::Result<Option<Revision>> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM revisions WHERE id = ?1"),
            [id],
            revision_row,
        )
        .optional()
}

/// The revision of a row of [`COLUMNS`].
fn revision_row(row: &Row<'_>) -> rusqlite::Result<Revision> {
    Ok(Revision {
        id: row.get(0)?,
        document_id: row.get(1)?,
        original_digest: digest(row, 2)?,
        canonical_digest: digest(row, 3)?,
        status: status(row, 4)?,
        captured_at: row.get(5)?,
        metadata: json(row, 6)?,
    })
}

/// The digest column `index` of `row` holds.
fn digest(row: &Row<'_>, index: usize) -> rusqlite::Result<Digest> {
    let text: String = row.get(index)?;
    Digest::parse(&text).map_err(|invalid| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(invalid))
    })
}

/// The status column `index` of `row` names.
fn status(row: &Row<'_>, index: usize) -> rusqlite::Result<RevisionStatus> {
    let text: String = row.get(index)?;
    RevisionStatus::ALL
        .into_iter()
        .find(|status| status.as_str() == text)
        .ok_or_else(|| {
            let unknown = format!("no revision status is named {text:?}");
            rusqlite::Error::FromSqlConversionFailure(index, Type::Text, unknown.into())
        })
}
