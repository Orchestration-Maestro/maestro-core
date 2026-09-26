//! The duplicates of revisions (docs/architecture/01 §6): the places each
//! revision's content occurs, and the groups of near duplicates, grouped and
//! never deleted.

use super::error::Error;
use crate::{scope::ScopeSet, store::Database};
use rusqlite::params;

/// A place a revision's content occurs: exact duplicates keep every one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    /// The revision whose content occurs there: the one prepared for every
    /// exact duplicate of it.
    pub revision_id: String,
    /// The collection of the place.
    pub collection_id: String,
    /// The source of that collection it occurs in.
    pub source_id: String,
    /// Where it comes from in that source.
    pub source_ref: String,
}

/// A member of a group of near duplicates: two versions of one topic, for
/// example, which retrieval may collapse.
#[derive(Debug, Clone, PartialEq)]
pub struct NearDuplicate {
    /// The group.
    pub group_id: String,
    /// The revision it holds.
    pub revision_id: String,
    /// The confirmed shingle Jaccard that holds the revision in the group,
    /// from 0 to 1.
    pub jaccard: f64,
}

impl Database {
    /// Records `occurrences` in one write; one recorded before is left as it
    /// is.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when a revision or a source is not recorded, or the
    /// database cannot record them: nothing is recorded then.
    pub fn record_occurrences(&self, occurrences: &[Occurrence]) -> Result<(), Error> {
        self.write(|transaction| {
            for occurrence in occurrences {
                transaction.execute(
                    "INSERT INTO occurrences (revision_id, collection_id, source_id, source_ref)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT DO NOTHING",
                    params![
                        occurrence.revision_id,
                        occurrence.collection_id,
                        occurrence.source_id,
                        occurrence.source_ref,
                    ],
                )?;
            }
            Ok(())
        })
    }

    /// Every place the content of the revision `revision_id` occurs in a
    /// source `scopes` covers, in source and source reference order, as
    /// every preparation recorded them: the duplicates of one chunk set are
    /// those its manifest lists.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn occurrences(
        &self,
        scopes: &ScopeSet,
        revision_id: &str,
    ) -> Result<Vec<Occurrence>, Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare(&format!(
            "SELECT revision_id, collection_id, source_id, source_ref FROM occurrences
             WHERE revision_id = ?1 AND {}
             ORDER BY collection_id, source_id, source_ref",
            ScopeSet::source_condition("occurrences.collection_id", "occurrences.source_id", 2)
        ))?;
        let occurrences = statement
            .query_map(params![revision_id, scopes.parameter()], |row| {
                Ok(Occurrence {
                    revision_id: row.get(0)?,
                    collection_id: row.get(1)?,
                    source_id: row.get(2)?,
                    source_ref: row.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(occurrences)
    }

    /// Records `members` of groups of near duplicates in one write; one
    /// recorded before is left as it is.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when a revision is not recorded, a Jaccard is not a
    /// number from 0 to 1, or the database cannot record them: nothing is
    /// recorded then.
    pub fn record_near_duplicates(&self, members: &[NearDuplicate]) -> Result<(), Error> {
        self.write(|transaction| {
            for member in members {
                transaction.execute(
                    "INSERT INTO near_dup_groups (group_id, revision_id, jaccard)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT DO NOTHING",
                    params![member.group_id, member.revision_id, member.jaccard],
                )?;
            }
            Ok(())
        })
    }

    /// Every member of every group of near duplicates that holds the revision
    /// `revision_id`, in group and revision order, when `scopes` covers the
    /// source of the revision: the members whose source it does not cover are
    /// left out. Every preparation's groups are here: the groups of one chunk
    /// set are those its manifest lists.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn near_duplicates(
        &self,
        scopes: &ScopeSet,
        revision_id: &str,
    ) -> Result<Vec<NearDuplicate>, Error> {
        let reader = self.reader()?;
        let visible =
            ScopeSet::source_condition("documents.collection_id", "documents.source_id", 2);
        let mut statement = reader.prepare(&format!(
            "SELECT near_dup_groups.group_id, near_dup_groups.revision_id, jaccard
             FROM near_dup_groups
             JOIN revisions ON revisions.id = near_dup_groups.revision_id
             JOIN documents ON documents.id = revisions.document_id
             WHERE {visible} AND near_dup_groups.group_id IN (
               SELECT near_dup_groups.group_id FROM near_dup_groups
               JOIN revisions ON revisions.id = near_dup_groups.revision_id
               JOIN documents ON documents.id = revisions.document_id
               WHERE near_dup_groups.revision_id = ?1 AND {visible})
             ORDER BY near_dup_groups.group_id, near_dup_groups.revision_id"
        ))?;
        let members = statement
            .query_map(params![revision_id, scopes.parameter()], |row| {
                Ok(NearDuplicate {
                    group_id: row.get(0)?,
                    revision_id: row.get(1)?,
                    jaccard: row.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(members)
    }
}
