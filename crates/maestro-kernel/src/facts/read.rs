//! Reading a claim set: whole, or not at all when the caller's scopes do not
//! cover its collection. Every revision its claims quote belongs to that
//! collection, so a scope that covers it covers their sources too.

use super::{
    error::Error,
    types::{
        Claim, ClaimRecord, ClaimSetRecord, EntityName, Literal, LiteralKind, Object, Provenance,
        ReviewState, Support, Validity,
    },
};
use crate::{
    artifact::Digest,
    evidence::Span,
    scope::ScopeSet,
    store::Database,
    vocabulary::{EntityKind, Predicate},
};
use rusqlite::{Connection, OptionalExtension as _, Row, params, types::Type};
use std::{collections::BTreeMap, error};

/// The columns of `claims` [`claim_row`] reads, in its order.
const CLAIM_COLUMNS: &str = "claims.id, claims.collection_id, subject_kind, subject_name,
    predicate, object_type, object_lexeme, conditions_json, version_known, version_start,
    version_end, world_known, world_start, world_end, extractor, profile_digest, review_state,
    recorded_at, object_kind, object_name";

impl Database {
    /// The claim set `id`, if `scopes` covers its collection; a set outside
    /// them reads as one that does not exist.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn claim_set(
        &self,
        scopes: &ScopeSet,
        id: &Digest,
    ) -> Result<Option<ClaimSetRecord>, Error> {
        load_set(&self.reader()?, scopes, id)
    }
}

/// [`Database::claim_set`], read through `connection`.
pub(super) fn load_set(
    connection: &Connection,
    scopes: &ScopeSet,
    id: &Digest,
) -> Result<Option<ClaimSetRecord>, Error> {
    let visible = connection
        .query_row(
            &format!(
                "SELECT collection_id FROM claim_sets WHERE id = ?1 AND {}",
                ScopeSet::collection_condition("collection_id", 2)
            ),
            params![id.as_str(), scopes.parameter()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(collection_id) = visible else {
        return Ok(None);
    };
    let mut statement = connection.prepare(&format!(
        "SELECT {CLAIM_COLUMNS} FROM claim_set_members
         JOIN claims ON claims.id = claim_set_members.claim_id
         WHERE claim_set_members.claim_set_id = ?1
         ORDER BY claim_set_members.ordinal"
    ))?;
    let mut claims = statement
        .query_map([id.as_str()], claim_row)?
        .collect::<Result<Vec<_>, _>>()?;
    for claim in &mut claims {
        claim.claim.supports = supports(connection, &claim.id)?;
    }
    Ok(Some(ClaimSetRecord {
        id: id.clone(),
        collection_id,
        claims,
    }))
}

/// The supports of the claim `claim_id`, in their canonical order.
fn supports(connection: &Connection, claim_id: &Digest) -> rusqlite::Result<Vec<Support>> {
    let mut statement = connection.prepare(
        "SELECT revision_id, block_id, span_start, span_end, quote_digest FROM claim_supports
         WHERE claim_id = ?1 ORDER BY revision_id, block_id, span_start, span_end",
    )?;
    statement
        .query_map([claim_id.as_str()], |row| {
            Ok(Support {
                revision_id: row.get(0)?,
                block_id: row.get(1)?,
                span: Span {
                    start: offset(row, 2)?,
                    end: offset(row, 3)?,
                },
                quote_digest: digest(row, 4)?,
            })
        })?
        .collect()
}

/// The claim a row of [`CLAIM_COLUMNS`] holds, with no supports yet.
fn claim_row(row: &Row<'_>) -> rusqlite::Result<ClaimRecord> {
    let conditions: String = row.get(7)?;
    Ok(ClaimRecord {
        id: digest(row, 0)?,
        collection_id: row.get(1)?,
        claim: Claim {
            subject: EntityName {
                kind: named(row, 2, EntityKind::parse)?,
                name: row.get(3)?,
            },
            predicate: named(row, 4, Predicate::parse)?,
            object: object(row)?,
            conditions: serde_json::from_str::<BTreeMap<String, String>>(&conditions)
                .map_err(|error| conversion(7, error))?,
            version: validity(row, 8)?,
            world: validity(row, 11)?,
            provenance: Provenance {
                extractor: row.get(14)?,
                profile: digest(row, 15)?,
            },
            supports: Vec::new(),
        },
        review: named(row, 16, ReviewState::parse)?,
        recorded_at: row.get(17)?,
    })
}

/// The object of a row of [`CLAIM_COLUMNS`]: its literal when its CHECK gave
/// it a type, which it does for `DEFAULTS_TO` alone, and its entity when not.
fn object(row: &Row<'_>) -> rusqlite::Result<Object> {
    Ok(if row.get::<_, Option<String>>(5)?.is_some() {
        Object::Literal(Literal {
            kind: named(row, 5, LiteralKind::parse)?,
            lexeme: row.get(6)?,
        })
    } else {
        Object::Entity(EntityName {
            kind: named(row, 18, EntityKind::parse)?,
            name: row.get(19)?,
        })
    })
}

/// The validity whose `known`, `start` and `end` columns start at `index`.
fn validity(row: &Row<'_>, index: usize) -> rusqlite::Result<Validity> {
    Ok(if row.get(index)? {
        Validity::Bounded {
            start: row.get(index + 1)?,
            end: row.get(index + 2)?,
        }
    } else {
        Validity::Unknown
    })
}

/// The byte offset in the column `index`, which its CHECK keeps at zero or
/// above.
fn offset(row: &Row<'_>, index: usize) -> rusqlite::Result<usize> {
    let value: i64 = row.get(index)?;
    usize::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

/// The digest in the column `index`.
fn digest(row: &Row<'_>, index: usize) -> rusqlite::Result<Digest> {
    let text: String = row.get(index)?;
    Digest::parse(&text).map_err(|invalid| conversion(index, invalid))
}

/// The value `parse` names by the text in the column `index`, which its
/// CHECK constraint keeps to the names `parse` knows.
fn named<T>(row: &Row<'_>, index: usize, parse: fn(&str) -> Option<T>) -> rusqlite::Result<T> {
    let text: String = row.get(index)?;
    parse(&text).ok_or_else(|| rusqlite::Error::InvalidColumnType(index, text, Type::Text))
}

/// The column `index` held text its type cannot read.
fn conversion(index: usize, error: impl error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
}

/// Hydrate a batch's immutable claims, in the order its receipt recorded.
pub(super) fn batch_claims(
    connection: &Connection,
    job: &str,
    ordinal: i64,
) -> Result<Vec<ClaimRecord>, Error> {
    let mut statement = connection.prepare(&format!(
        "SELECT {CLAIM_COLUMNS} FROM graph_build_claims b
         JOIN claims ON claims.id = b.claim_id
         WHERE b.job_id = ?1 AND b.ordinal = ?2 ORDER BY b.position"
    ))?;
    let mut claims = statement
        .query_map(params![job, ordinal], claim_row)?
        .collect::<Result<Vec<_>, _>>()?;
    for claim in &mut claims {
        claim.claim.supports = supports(connection, &claim.id)?;
    }
    Ok(claims)
}
