//! One strict scoped decoder for immutable 0031 build receipts.
use super::{
    build_types::ProjectionReceiptIdentity,
    error::Error,
    projection_binding::{EXACT_RESOLVER_VERSION, PROJECTION_REBUILD_REPAIR},
};
use crate::{artifact::Digest, scope::ScopeSet};
use rusqlite::{Connection, OptionalExtension as _, params};

/// Stored projection-readiness receipt columns before decoding.
pub(super) type ProjectionReceiptRow = (
    String,
    String,
    String,
    String,
    i64,
    i64,
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Strict version-2/3 pins; legacy identities have no such tuple.
pub(super) type ProjectionPins = (Digest, String, Digest, Digest);

/// Decode common fields before classifying the format or validating pins.
pub(super) fn decode_identity(
    generation_id: i64,
    build_id: i64,
    row: ProjectionReceiptRow,
) -> Result<(ProjectionReceiptIdentity, Option<ProjectionPins>), Error> {
    let (
        collection_id,
        set,
        file_name,
        schema_version,
        edges,
        catalog_edges,
        facts,
        digest,
        resolution,
        resolver,
        settings,
        lock,
    ) = row;
    if generation_id <= 0
        || build_id <= 0
        || !file_name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !file_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(Error::Conflict("invalid projection file identity".into()));
    }
    let identity = ProjectionReceiptIdentity {
        build_id,
        collection_id,
        generation_id,
        file_name,
        schema_version,
        claim_set_id: Digest::parse(&set).map_err(|error| {
            Error::Conflict(format!("invalid projection claim-set id: {error}"))
        })?,
        knowledge_edge_count: usize::try_from(edges)
            .map_err(|_| Error::Conflict("invalid projection edge count".to_owned()))?,
        catalog_dependency_edge_count: usize::try_from(catalog_edges)
            .map_err(|_| Error::Conflict("invalid projection catalog count".to_owned()))?,
        entity_fact_count: usize::try_from(facts)
            .map_err(|_| Error::Conflict("invalid projection fact count".to_owned()))?,
        content_digest: Digest::parse(&digest).map_err(|error| {
            Error::Conflict(format!("invalid projection content digest: {error}"))
        })?,
    };
    let invalid = || {
        Error::Conflict(format!(
            "invalid projection input pins; {PROJECTION_REBUILD_REPAIR}"
        ))
    };
    if identity.schema_version == "maestro-typed-edges/1" {
        if resolution.is_some() || resolver.is_some() || settings.is_some() || lock.is_some() {
            return Err(invalid());
        }
        return Ok((identity, None));
    }
    if (identity.schema_version != "maestro-typed-edges/2"
        && identity.schema_version != "maestro-typed-edges/3")
        || resolver.as_deref() != Some(EXACT_RESOLVER_VERSION)
    {
        return Err(invalid());
    }
    let pin = |value: Option<String>| {
        value
            .and_then(|value| Digest::parse(&value).ok())
            .ok_or_else(invalid)
    };
    let pins = (
        pin(resolution)?,
        resolver.ok_or_else(invalid)?,
        pin(settings)?,
        pin(lock)?,
    );
    Ok((identity, Some(pins)))
}

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
        let (identity, pins) = decode_identity(generation, build, row)?;
        if identity.collection_id != collection {
            return Err(Error::Conflict("invalid projection build identity".into()));
        }
        Ok(Record { identity, pins })
    })
    .transpose()
}

/// Read an active selection (or the published inventory) in one joined snapshot.
/// A missing head is readiness absence; a broken head is never an absent receipt.
pub(super) fn active(
    connection: &Connection,
    scopes: &ScopeSet,
    generation: Option<i64>,
) -> Result<Vec<(String, i64, Option<Record>)>, Error> {
    let mut statement = connection.prepare(&format!(
        "SELECT g.collection_id, g.id, a.build_id, b.build_id, b.generation_id,
                b.collection_id, b.claim_set_id, r.file_name, b.schema_version,
                r.knowledge_edge_count, r.catalog_dependency_edge_count,
                r.entity_fact_count, r.content_digest, b.resolution_id,
                b.resolver_version, b.settings_identity, b.frozen_lock
         FROM generations g
         LEFT JOIN graph_projection_active a ON a.generation_id = g.id
         LEFT JOIN graph_projection_builds b ON b.build_id = a.build_id
         LEFT JOIN graph_projection_receipts r ON r.build_id = a.build_id
         WHERE ((?1 IS NULL AND g.state = 'published') OR g.id = ?1) AND {}
         ORDER BY g.collection_id, g.id",
        ScopeSet::collection_condition("g.collection_id", 2)
    ))?;
    let rows = statement.query_map(params![generation, scopes.parameter()], |row| {
        let head = row.get::<_, Option<i64>>(2)?;
        let build = row.get::<_, Option<i64>>(3)?;
        let target = row.get::<_, Option<i64>>(4)?;
        let name = row.get::<_, Option<String>>(7)?;
        let receipt = if head.is_some() && build.is_some() && name.is_some() {
            Some((
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
                row.get(13)?,
                row.get(14)?,
                row.get(15)?,
                row.get(16)?,
            ))
        } else {
            None
        };
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            head,
            build,
            target,
            receipt,
        ))
    })?;
    rows.map(|row| {
        let (collection, generation, head, build, target, receipt) = row?;
        let record = match head {
            None => None,
            Some(head) => {
                if build != Some(head) || target != Some(generation) {
                    return Err(Error::Conflict("dangling projection active head".into()));
                }
                let row = receipt
                    .ok_or_else(|| Error::Conflict("unreceipted projection active head".into()))?;
                let (identity, pins) = decode_identity(generation, head, row)?;
                if identity.collection_id != collection {
                    return Err(Error::Conflict("invalid projection active identity".into()));
                }
                Some(Record { identity, pins })
            }
        };
        Ok((collection, generation, record))
    })
    .collect()
}

impl Record {
    /// Require complete durable pins for a native-readable receipt.
    pub(super) fn receipt(self) -> Result<super::build_types::ProjectionReceipt, Error> {
        let (resolution_id, resolver_version, settings_identity, frozen_lock) = self.pins.ok_or(
            Error::ProjectionInputMismatch(super::projection_binding::InputMismatchKind::Format),
        )?;
        Ok(super::build_types::ProjectionReceipt {
            identity: self.identity,
            resolution_id,
            resolver_version,
            settings_identity,
            frozen_lock,
        })
    }
}
