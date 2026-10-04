//! One strict scoped decoder for immutable 0031 build receipts.
use super::{
    build_types::ProjectionReceiptIdentity,
    error::Error,
    projection::{ProjectionPins, decode_identity},
};
use crate::scope::ScopeSet;
use rusqlite::{Connection, OptionalExtension as _, params};

/// Retained output, including absent pins on genuine legacy records.
pub(super) struct Record {
    /// Strict file and canonical content identity.
    pub(super) identity: ProjectionReceiptIdentity,
    /// Input pins, absent only for format /1.
    pub(super) pins: Option<ProjectionPins>,
}

/// Look up an exact generation/build pair under the caller's current scope.
pub(super) fn by_build(
    connection: &Connection,
    scopes: &ScopeSet,
    generation: i64,
    build: i64,
) -> Result<Option<Record>, Error> {
    let row = connection
        .query_row(
            &format!(
                "SELECT b.collection_id, b.claim_set_id, r.file_name, b.schema_version,
                    r.knowledge_edge_count, r.catalog_dependency_edge_count,
                    r.entity_fact_count, r.content_digest, b.resolution_id,
                    b.resolver_version, b.settings_identity, b.frozen_lock, g.collection_id
             FROM graph_projection_builds b
             JOIN graph_projection_receipts r ON r.build_id = b.build_id
             JOIN generations g ON g.id = b.generation_id
             WHERE b.generation_id = ?1 AND b.build_id = ?2 AND {}",
                ScopeSet::collection_condition("g.collection_id", 3)
            ),
            params![generation, build, scopes.parameter()],
            |row| {
                Ok((
                    (
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                    ),
                    row.get::<_, String>(12)?,
                ))
            },
        )
        .optional()?;
    row.map(|(row, collection)| {
        let (identity, pins) = decode_identity(generation, row)?;
        if build <= 0 || identity.collection_id != collection {
            return Err(Error::Conflict("invalid projection build identity".into()));
        }
        Ok(Record { identity, pins })
    })
    .transpose()
}
