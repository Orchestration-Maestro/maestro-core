//! Immutable generation attachments, scoped to the build's collection.

use super::{build_read::load_build, build_types::GraphAttachment, error::Error};
use crate::{
    artifact::Digest,
    job::{self, Lease},
    scope::ScopeSet,
    store::Database,
};
use rusqlite::{Connection, OptionalExtension as _, params};

impl Database {
    /// Attach a finished build to a building or verified generation, once.
    /// An identical attachment is a no-op, including after publication, while
    /// the caller still holds the job lease.
    ///
    /// # Errors
    /// Refuses unknown or unfinished builds, different attachments, other
    /// collections, published generations, lost leases and database errors.
    pub fn attach_claim_set(
        &self,
        scopes: &ScopeSet,
        generation: i64,
        lease: &Lease,
    ) -> Result<GraphAttachment, Error> {
        let job = lease.job;
        self.write(|transaction| {
            let build = load_build(transaction, scopes, job)?.ok_or(Error::UnknownBuild(job))?;
            job::validate_lease(transaction, lease)?;
            let set = build.claim_set_id.ok_or(Error::Unfinished {
                recorded: build.batches.len(),
                expected: build.plan.sources.len(),
            })?;
            let attachment = GraphAttachment {
                generation_id: generation,
                job,
                claim_set_id: set,
            };
            if let Some(existing) = load_attachment(transaction, scopes, generation)? {
                return (existing == attachment)
                    .then_some(existing)
                    .ok_or_else(|| Error::Conflict("generation already has another graph".into()));
            }
            let row: Option<(String, String)> = transaction
                .query_row(
                    "SELECT collection_id, state FROM generations WHERE id = ?1",
                    [generation],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((collection, state)) = row else {
                return Err(Error::Conflict("unknown generation".into()));
            };
            if collection != build.plan.collection_id
                || !matches!(state.as_str(), "building" | "verified")
            {
                return Err(Error::Conflict(
                    "generation cannot take this attachment".into(),
                ));
            }
            transaction.execute(
                "INSERT INTO graph_attachments (generation_id, job_id, claim_set_id)
                 VALUES (?1, ?2, ?3)",
                params![
                    generation,
                    job.to_string(),
                    attachment.claim_set_id.as_str()
                ],
            )?;
            Ok(attachment)
        })
    }

    /// Read the generation's attachment, or none when its collection is not visible.
    ///
    /// # Errors
    /// Returns [`Error::Store`] for an unreadable database or [`Error::Conflict`]
    /// for corrupt stored identities.
    pub fn graph_attachment(
        &self,
        scopes: &ScopeSet,
        generation: i64,
    ) -> Result<Option<GraphAttachment>, Error> {
        load_attachment(&self.reader()?, scopes, generation)
    }
}

/// Read an attachment through the caller's transaction or reader.
fn load_attachment(
    connection: &Connection,
    scopes: &ScopeSet,
    generation: i64,
) -> Result<Option<GraphAttachment>, Error> {
    let row: Option<(String, String)> = connection
        .query_row(
            &format!(
                "SELECT a.job_id, a.claim_set_id FROM graph_attachments a
         JOIN generations g ON g.id = a.generation_id WHERE a.generation_id = ?1 AND {}",
                ScopeSet::collection_condition("g.collection_id", 2)
            ),
            params![generation, scopes.parameter()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    row.map(|(job, set)| {
        Ok(GraphAttachment {
            generation_id: generation,
            job: job
                .parse()
                .map_err(|_| Error::Conflict("invalid job id".into()))?,
            claim_set_id: Digest::parse(&set)
                .map_err(|_| Error::Conflict("invalid set id".into()))?,
        })
    })
    .transpose()
}
