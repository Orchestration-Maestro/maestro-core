//! Scoped, transaction-consistent hydration of durable graph builds.

use super::{
    build_types::{BatchReceipt, Budget, BuildPlan, BuildRecord, Rejection},
    error::Error,
    read::batch_claims,
    types::Provenance,
};
use crate::{artifact::Digest, scope::ScopeSet, store::Database};
use rusqlite::{Connection, OptionalExtension as _, Row, params};
use ulid::Ulid;

impl Database {
    /// Read a build and all its receipts; a build outside `scopes` is unknown.
    ///
    /// # Errors
    /// Returns [`Error::Store`] for an unreadable database.
    pub fn graph_build(&self, scopes: &ScopeSet, job: Ulid) -> Result<Option<BuildRecord>, Error> {
        let mut connection = self.reader()?;
        let transaction = connection.transaction()?;
        load_build(&transaction, scopes, job)
    }
}

/// Hydrate a build inside the caller's read snapshot or write transaction.
pub(super) fn load_build(
    connection: &Connection,
    scopes: &ScopeSet,
    job: Ulid,
) -> Result<Option<BuildRecord>, Error> {
    let row = connection
        .query_row(
            &format!(
                "SELECT collection_id, extractor, profile_digest, sources_json,
          max_claims, max_rejections, claim_set_id FROM graph_builds WHERE job_id = ?1 AND {}",
                ScopeSet::collection_condition("collection_id", 2)
            ),
            params![job.to_string(), scopes.parameter()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    count(row, 4)?,
                    count(row, 5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((collection_id, extractor, profile, sources, max_claims, max_rejections, set)) = row
    else {
        return Ok(None);
    };
    let plan = BuildPlan {
        collection_id,
        provenance: Provenance {
            extractor,
            profile: parse_digest(&profile)?,
        },
        sources: serde_json::from_str(&sources)
            .map_err(|_| Error::Invalid("invalid source list".into()))?,
        budget: Budget {
            max_claims,
            max_rejections,
        },
    };
    let mut statement = connection.prepare(
        "SELECT ordinal, revision_id, lease_number, rejected, kept FROM graph_build_batches
         WHERE job_id = ?1 ORDER BY ordinal",
    )?;
    let mut batches = statement
        .query_map([job.to_string()], |row| {
            Ok(BatchReceipt {
                ordinal: count(row, 0)?,
                revision_id: row.get(1)?,
                lease_number: row.get::<_, i64>(2)?.unsigned_abs(),
                claims: Vec::new(),
                rejected: count(row, 3)?,
                kept: count(row, 4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for batch in &mut batches {
        batch.claims = batch_claims(connection, &job.to_string(), integer(batch.ordinal)?)?;
    }
    let mut statement = connection.prepare(
        "SELECT revision_id, block_id, reason FROM graph_build_rejections
         WHERE job_id = ?1 ORDER BY position",
    )?;
    let rejections = statement
        .query_map([job.to_string()], |row| {
            Ok(Rejection {
                revision_id: row.get(0)?,
                block_id: row.get(1)?,
                reason: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(BuildRecord {
        plan,
        batches,
        rejections,
        claim_set_id: set.as_deref().map(parse_digest).transpose()?,
    }))
}

/// Parse a durable digest without panicking on database corruption.
fn parse_digest(text: &str) -> Result<Digest, Error> {
    Digest::parse(text).map_err(|_| Error::Invalid("invalid stored digest".into()))
}

/// Reject an input count SQLite cannot represent, before writing it.
pub(super) fn integer(value: usize) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::Invalid("count exceeds SQLite range".into()))
}

/// Read a nonnegative SQLite count without truncating it on narrower hosts.
fn count(row: &Row<'_>, index: usize) -> rusqlite::Result<usize> {
    let value: i64 = row.get(index)?;
    usize::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
