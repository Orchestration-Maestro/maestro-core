//! Transactional receipt and head publication for reserved 0031 builds.
use super::{
    build_types::ProjectionReceipt,
    error::Error,
    projection_records,
    projection_reservation::{self, Request},
};
use crate::{job::Lease, scope::ScopeSet};
use rusqlite::{Transaction, params};
use std::time::SystemTime;

/// Insert verified output and select it under the current build-specific lease.
pub(super) fn publish(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    output: (i64, &ProjectionReceipt),
    lease: &Lease,
    now: SystemTime,
) -> Result<(), Error> {
    let (build, receipt) = output;
    let reserved = projection_reservation::validate(transaction, scopes, build, lease, now)?;
    validate_output(transaction, scopes, &reserved.request, receipt)?;
    let identity = &receipt.identity;
    let count = |value| {
        i64::try_from(value).map_err(|_| Error::Conflict("projection count is too large".into()))
    };
    transaction.execute(
        "INSERT INTO graph_projection_receipts
         (build_id, file_name, knowledge_edge_count, catalog_dependency_edge_count,
          entity_fact_count, content_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            build,
            identity.file_name,
            count(identity.knowledge_edge_count)?,
            count(identity.catalog_dependency_edge_count)?,
            count(identity.entity_fact_count)?,
            identity.content_digest.as_str()
        ],
    )?;
    let changed = match reserved.request.expected_active_build_id {
        Some(previous) => transaction.execute(
            "UPDATE graph_projection_active SET build_id = ?1
             WHERE generation_id = ?2 AND build_id = ?3",
            params![build, identity.generation_id, previous],
        )?,
        None => transaction.execute(
            "INSERT INTO graph_projection_active (generation_id, build_id)
             SELECT ?1, ?2 WHERE NOT EXISTS
             (SELECT 1 FROM graph_projection_active WHERE generation_id = ?1)",
            params![identity.generation_id, build],
        )?,
    };
    if changed != 1 {
        return Err(Error::Conflict("projection active head changed".into()));
    }
    Ok(())
}

/// Verify every supplied input and pinned predecessor content before insertion.
fn validate_output(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    request: &Request,
    receipt: &ProjectionReceipt,
) -> Result<(), Error> {
    let identity = &receipt.identity;
    if identity.collection_id != request.collection_id
        || identity.generation_id != request.generation_id
        || identity.claim_set_id != request.claim_set_id
        || identity.schema_version != "maestro-typed-edges/3"
        || receipt.resolution_id != request.resolution_id
        || receipt.resolver_version != request.resolver_version
        || receipt.settings_identity != request.settings_identity
        || receipt.frozen_lock != request.frozen_lock
    {
        return Err(Error::Conflict(
            "projection output differs from reserved inputs".into(),
        ));
    }
    if let Some(previous) = request.expected_active_build_id {
        let old =
            projection_records::by_build(transaction, scopes, request.generation_id, previous)?
                .ok_or(Error::Unauthorized)?;
        if old.pins.is_some()
            && (old.identity.knowledge_edge_count != identity.knowledge_edge_count
                || old.identity.catalog_dependency_edge_count
                    != identity.catalog_dependency_edge_count
                || old.identity.entity_fact_count != identity.entity_fact_count
                || old.identity.content_digest != identity.content_digest)
        {
            return Err(Error::Conflict(
                "projection output differs from pinned predecessor content".into(),
            ));
        }
    }
    Ok(())
}
