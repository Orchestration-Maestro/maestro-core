//! Quality dispositions: the one outcome a revision is given before it may be
//! indexed (docs/architecture/01 §4), kept once given, with the event of a
//! revision held back journaled in the write that records its disposition,
//! and a revision recorded with its disposition in one write when it must
//! never be read undecided.

use super::{
    collection::json,
    error::Error,
    revision::{self, Recorded, Revision},
};
use crate::{
    journal::{HeldDisposition, NewEvent, RevisionHeld, event, stream},
    scope::{ScopeSet, source_path},
    store::Database,
};
use rusqlite::{OptionalExtension as _, Row, Transaction, params, types::Type};
use serde_json::{Value, json};
use std::fmt;

/// A revision's quality disposition: its outcome, why, and who decided it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disposition {
    /// The revision it decides.
    pub revision_id: String,
    /// The outcome.
    pub outcome: Outcome,
    /// Why, for people.
    pub reasons: Vec<String>,
    /// The rules that gave it.
    pub rule_ids: Vec<String>,
    /// Who decided it: the quality gate, the import or a person.
    pub decided_by: String,
}

/// The outcome of the quality gate for a revision (01 §4): the first two let
/// it be indexed, the other three hold it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It passed the automatic checks and the collection's rules.
    Accepted,
    /// It is accepted under an explicit policy that keeps its warnings
    /// visible in evidence.
    AcceptedWithWarnings,
    /// Its structure was lost or garbled: it waits for another extraction.
    NeedsReextraction,
    /// It needs a review: a suspected secret, hostile content or unresolved
    /// provenance.
    Quarantined,
    /// It is out of scope by a recorded decision.
    Excluded,
}

impl Outcome {
    /// Every outcome.
    pub(super) const ALL: [Self; 5] = [
        Self::Accepted,
        Self::AcceptedWithWarnings,
        Self::NeedsReextraction,
        Self::Quarantined,
        Self::Excluded,
    ];

    /// Its name, as the `disposition` column holds it.
    fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::AcceptedWithWarnings => "accepted_with_warnings",
            Self::NeedsReextraction => "needs_reextraction",
            Self::Quarantined => "quarantined",
            Self::Excluded => "excluded",
        }
    }

    /// The disposition its `revision.held` event names, when it holds its
    /// revision back from indexing; none when it lets it be indexed.
    fn held(self) -> Option<HeldDisposition> {
        match self {
            Self::Accepted | Self::AcceptedWithWarnings => None,
            Self::NeedsReextraction => Some(HeldDisposition::NeedsReextraction),
            Self::Quarantined => Some(HeldDisposition::Quarantined),
            Self::Excluded => Some(HeldDisposition::Excluded),
        }
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Database {
    /// Records `disposition`, unless its revision has one already, which it
    /// keeps whatever this one says: a revision is decided once. A
    /// disposition that holds its revision back is journaled in the same
    /// write as `maestro.knowledge.revision.held.v1`, on the stream of the
    /// revision's collection ([`stream`]), in the scope of its document's
    /// source.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when its revision is not recorded, when the journal
    /// refuses its event, or when the database cannot record it: nothing is
    /// recorded then.
    pub fn record_disposition(&self, disposition: &Disposition) -> Result<Recorded, Error> {
        self.write(|transaction| record(transaction, disposition))
    }

    /// Records `revision` as [`Database::record_revision`] does and
    /// `disposition` as [`Database::record_disposition`] does, in one write:
    /// both, or neither. A revision recorded with a disposition that holds it
    /// back is never read undecided, and no crash leaves it so. Returns what
    /// recording the disposition did: [`Recorded::New`] when it is recorded
    /// now, whether the revision is new or was recorded before.
    ///
    /// # Errors
    ///
    /// [`Error::RevisionConflict`] when the revision's id is recorded with
    /// other content, and [`Error::Store`] when an artifact, the revision's
    /// document or the disposition's revision is not recorded, when the
    /// journal refuses the hold's event, or when the database cannot record
    /// them: nothing is recorded then.
    pub fn record_revision_with_disposition(
        &self,
        revision: &Revision,
        disposition: &Disposition,
    ) -> Result<Recorded, Error> {
        self.write(|transaction| {
            revision::record(transaction, revision)?;
            record(transaction, disposition)
        })
    }

    /// The disposition of the revision `revision_id`, if it has one and
    /// `scopes` covers the scope of its document's source.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn disposition(
        &self,
        scopes: &ScopeSet,
        revision_id: &str,
    ) -> Result<Option<Disposition>, Error> {
        let disposition = self
            .reader()?
            .query_row(
                &format!(
                    "SELECT revision_id, disposition, reasons_json, rule_ids, decided_by
                     FROM quality_dispositions
                     JOIN revisions ON revisions.id = quality_dispositions.revision_id
                     JOIN documents ON documents.id = revisions.document_id
                     WHERE revision_id = ?1 AND {}",
                    ScopeSet::source_condition("documents.collection_id", "documents.source_id", 2)
                ),
                params![revision_id, scopes.parameter()],
                disposition_row,
            )
            .optional()?;
        Ok(disposition)
    }
}

/// Records `disposition` inside `transaction`, unless its revision has one
/// already, and journals it there when it holds its revision back.
fn record(transaction: &Transaction<'_>, disposition: &Disposition) -> Result<Recorded, Error> {
    let inserted = transaction.execute(
        "INSERT INTO quality_dispositions (revision_id, disposition, reasons_json, rule_ids,
           decided_by)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (revision_id) DO NOTHING",
        params![
            disposition.revision_id,
            disposition.outcome.as_str(),
            json_list(&disposition.reasons),
            json_list(&disposition.rule_ids),
            disposition.decided_by,
        ],
    )?;
    if inserted == 0 {
        return Ok(Recorded::Unchanged);
    }
    if let Some(held) = disposition.outcome.held() {
        journal_hold(transaction, &disposition.revision_id, held)?;
    }
    Ok(Recorded::New)
}

/// Journals inside `transaction` that the revision `revision_id` is held
/// back as `held`: on the stream of its collection, in the scope of its
/// document's source.
fn journal_hold(
    transaction: &Transaction<'_>,
    revision_id: &str,
    held: HeldDisposition,
) -> Result<(), Error> {
    let (collection, source): (String, String) = transaction.query_row(
        "SELECT documents.collection_id, documents.source_id FROM revisions
         JOIN documents ON documents.id = revisions.document_id WHERE revisions.id = ?1",
        [revision_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let scope = source_path(&collection, &source);
    let data = json!(RevisionHeld {
        collection: collection.clone(),
        revision: revision_id.to_owned(),
        disposition: held,
    });
    event::record(
        transaction,
        &NewEvent {
            stream: &stream(&collection),
            r#type: RevisionHeld::TYPE,
            subject: &format!("revision/{revision_id}"),
            scope: &scope,
            data: &data,
        },
    )?;
    Ok(())
}

/// `items` as the JSON array of strings their column holds.
fn json_list(items: &[String]) -> String {
    Value::from_iter(items.iter().map(String::as_str)).to_string()
}

/// The disposition of a row of `quality_dispositions`.
fn disposition_row(row: &Row<'_>) -> rusqlite::Result<Disposition> {
    Ok(Disposition {
        revision_id: row.get(0)?,
        outcome: outcome(row, 1)?,
        reasons: json(row, 2)?,
        rule_ids: json(row, 3)?,
        decided_by: row.get(4)?,
    })
}

/// The outcome column `index` of `row` names.
pub(super) fn outcome(row: &Row<'_>, index: usize) -> rusqlite::Result<Outcome> {
    let text: String = row.get(index)?;
    Outcome::ALL
        .into_iter()
        .find(|outcome| outcome.as_str() == text)
        .ok_or_else(|| {
            let unknown = format!("no quality disposition is named {text:?}");
            rusqlite::Error::FromSqlConversionFailure(index, Type::Text, unknown.into())
        })
}
