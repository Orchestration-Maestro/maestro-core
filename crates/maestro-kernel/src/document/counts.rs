//! A collection's counts: its documents, and its revisions by
//! canonicalization's status and by quality disposition, as the caller's
//! scopes cover them, which `maestro knowledge status` reports.

use super::{
    disposition::{Outcome, outcome},
    error::Error,
    revision::{RevisionStatus, status},
};
use crate::{scope::ScopeSet, store::Database};
use rusqlite::{Row, params};

/// How many of a collection's records the caller's scopes cover: those of
/// the documents whose source has a scope in the set, read in one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counts {
    /// Its documents.
    pub documents: u64,
    /// Its revisions with each status: every status, in the order valid,
    /// valid with warnings, failed, with zero for a status no revision has.
    pub statuses: Vec<(RevisionStatus, u64)>,
    /// Its revisions with each quality outcome: every outcome, in the order
    /// of [`Outcome`]'s variants, with zero for an outcome no revision has.
    pub outcomes: Vec<(Outcome, u64)>,
    /// Its revisions the quality gate has not decided yet.
    pub undecided: u64,
}

impl Counts {
    /// The counts of `documents` documents and no revision.
    fn of(documents: u64) -> Self {
        Self {
            documents,
            statuses: RevisionStatus::ALL.map(|status| (status, 0)).to_vec(),
            outcomes: Outcome::ALL.map(|outcome| (outcome, 0)).to_vec(),
            undecided: 0,
        }
    }

    /// Counts `number` more revisions with `status`, decided as `outcome`
    /// if they are.
    fn add(&mut self, status: RevisionStatus, outcome: Option<Outcome>, number: u64) {
        add_to(&mut self.statuses, &status, number);
        match outcome {
            Some(outcome) => add_to(&mut self.outcomes, &outcome, number),
            None => self.undecided += number,
        }
    }
}

/// Adds `number` to the count of `key` in `counts`.
fn add_to<K: PartialEq>(counts: &mut [(K, u64)], key: &K, number: u64) {
    for (each, count) in counts {
        if each == key {
            *count += number;
        }
    }
}

impl Database {
    /// The counts of the records of the collection `collection_id` whose
    /// document's source has a scope `scopes` covers: its documents, and its
    /// revisions by status and by quality disposition, those without one
    /// counted apart. A collection nothing is recorded for, or none the set
    /// covers, counts zero of everything.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read, or holds a status
    /// or a disposition no revision can have.
    pub fn collection_counts(
        &self,
        scopes: &ScopeSet,
        collection_id: &str,
    ) -> Result<Counts, Error> {
        let mut reader = self.reader()?;
        // One read transaction, so both queries see the same commit.
        let snapshot = reader.transaction()?;
        let covered =
            ScopeSet::source_condition("documents.collection_id", "documents.source_id", 2);
        let parameters = params![collection_id, scopes.parameter()];
        let documents = snapshot.query_row(
            &format!("SELECT count(*) FROM documents WHERE collection_id = ?1 AND {covered}"),
            parameters,
            |row| count(row, 0),
        )?;
        let mut counts = Counts::of(documents);
        let mut groups = snapshot.prepare(&format!(
            "SELECT revisions.status, quality_dispositions.disposition, count(*)
             FROM revisions JOIN documents ON documents.id = revisions.document_id
             LEFT JOIN quality_dispositions ON quality_dispositions.revision_id = revisions.id
             WHERE documents.collection_id = ?1 AND {covered}
             GROUP BY revisions.status, quality_dispositions.disposition"
        ))?;
        let rows = groups.query_map(parameters, |row| {
            let decided: Option<String> = row.get(1)?;
            let decision = decided.map(|_| outcome(row, 1)).transpose()?;
            Ok((status(row, 0)?, decision, count(row, 2)?))
        })?;
        for row in rows {
            let (status, decision, number) = row?;
            counts.add(status, decision, number);
        }
        Ok(counts)
    }
}

/// The count in column `index` of `row`, which SQLite never gives below
/// zero.
fn count(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
