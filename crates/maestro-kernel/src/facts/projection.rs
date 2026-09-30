//! Kernel-controlled verification receipts for immutable graph projections.

use super::{build_types::ProjectionReceipt, error::Error};
use crate::{
    artifact::Digest,
    job::{self, Lease, NewJob},
    scope::{ScopeSet, collection_path},
    store::Database,
};
use rusqlite::{OptionalExtension as _, params};
use serde_json::json;

/// Stored projection-readiness receipt columns before decoding.
type ProjectionReceiptRow = (String, String, String, String, i64, i64, i64, String);

impl Database {
    /// Record projection readiness for a verified, attached generation. The
    /// kernel rechecks knowledge-edge and fact counts; catalog counts come from
    /// the verified backend build.
    ///
    /// # Errors
    /// Refuses unauthorized scopes, expired or mismatched project leases,
    /// invalid receipt identities, unattached or unverified generations,
    /// count mismatches, duplicate receipts, and store failures.
    pub fn record_projection_ready(
        &self,
        scopes: &ScopeSet,
        receipt: &ProjectionReceipt,
        lease: &Lease,
    ) -> Result<(), Error> {
        if receipt.generation_id <= 0
            || !receipt
                .file_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            || !receipt
                .file_name
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            || receipt.schema_version != "maestro-typed-edges/1"
        {
            return Err(Error::Conflict(
                "invalid projection receipt identity".to_owned(),
            ));
        }
        self.write(|transaction| {
            let holder = job::validate_lease(transaction, lease)?;
            let inputs = json!({"generation": receipt.generation_id});
            let expected_job = NewJob {
                kind: "knowledge.graph.project",
                inputs: &inputs,
                scope: &holder.scope,
                resource: None,
            };
            if holder.scope.as_str() != collection_path(&receipt.collection_id)
                || holder.kind != expected_job.kind
                || holder.idempotency_key != job::idempotency_key(&expected_job)
            {
                return Err(Error::Unauthorized);
            }
            let expected: Option<(String, i64, i64)> = transaction
                .query_row(
                    &format!(
                        "SELECT a.claim_set_id,
                          (SELECT count(*) FROM claim_set_members m
                           JOIN claims c ON c.id = m.claim_id
                           WHERE m.claim_set_id = a.claim_set_id AND c.object_kind IS NOT NULL),
                          (SELECT count(*) FROM claim_set_members m
                           JOIN claims c ON c.id = m.claim_id
                           WHERE m.claim_set_id = a.claim_set_id AND c.object_type IS NOT NULL)
                         FROM graph_attachments a JOIN generations g ON g.id = a.generation_id
                         WHERE g.id = ?1 AND g.collection_id = ?2 AND g.state = 'verified' AND {}",
                        ScopeSet::collection_condition("g.collection_id", 3)
                    ),
                    params![
                        receipt.generation_id,
                        receipt.collection_id,
                        scopes.parameter()
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let Some((claim_set, edges, facts)) = expected else {
                return Err(Error::Unauthorized);
            };
            if claim_set != receipt.claim_set_id.as_str()
                || usize::try_from(edges).ok() != Some(receipt.knowledge_edge_count)
                || usize::try_from(facts).ok() != Some(receipt.entity_fact_count)
            {
                return Err(Error::Conflict(
                    "projection receipt differs from its authoritative claim set".to_owned(),
                ));
            }
            let already_ready: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_projection_receipts WHERE generation_id = ?1)",
                [receipt.generation_id],
                |row| row.get(0),
            )?;
            if already_ready {
                return Err(Error::Conflict(
                    "projection readiness is already recorded".to_owned(),
                ));
            }
            let edge_count = i64::try_from(receipt.knowledge_edge_count)
                .map_err(|_| Error::Conflict("projection edge count is too large".to_owned()))?;
            let catalog_count = i64::try_from(receipt.catalog_dependency_edge_count)
                .map_err(|_| Error::Conflict("projection catalog count is too large".to_owned()))?;
            let fact_count = i64::try_from(receipt.entity_fact_count)
                .map_err(|_| Error::Conflict("projection fact count is too large".to_owned()))?;
            transaction.execute(
                "INSERT INTO graph_projection_receipts
                 (generation_id, collection_id, claim_set_id, file_name, schema_version,
                  knowledge_edge_count, catalog_dependency_edge_count,
                  entity_fact_count, content_digest)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    receipt.generation_id,
                    receipt.collection_id,
                    receipt.claim_set_id.as_str(),
                    receipt.file_name,
                    receipt.schema_version,
                    edge_count,
                    catalog_count,
                    fact_count,
                    receipt.content_digest.as_str(),
                ],
            )?;
            Ok(())
        })
    }

    /// Read the recorded readiness for a generation visible to `scopes`.
    ///
    /// # Errors
    /// Returns a store error for unreadable or malformed receipt data.
    pub fn projection_ready(
        &self,
        scopes: &ScopeSet,
        generation: i64,
    ) -> Result<Option<ProjectionReceipt>, Error> {
        let row: Option<ProjectionReceiptRow> = self
            .reader()?
            .query_row(
                &format!(
                    "SELECT r.collection_id, r.claim_set_id, r.file_name, r.schema_version,
                            r.knowledge_edge_count, r.catalog_dependency_edge_count,
                            r.entity_fact_count, r.content_digest
                     FROM graph_projection_receipts r
                     JOIN generations g ON g.id = r.generation_id
                     WHERE r.generation_id = ?1 AND {}",
                    ScopeSet::collection_condition("g.collection_id", 2)
                ),
                params![generation, scopes.parameter()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .optional()?;
        row.map(
            |(
                collection_id,
                set,
                file_name,
                schema_version,
                edges,
                catalog_edges,
                facts,
                digest,
            )| {
                Ok(ProjectionReceipt {
                    collection_id,
                    generation_id: generation,
                    claim_set_id: Digest::parse(&set).map_err(|error| {
                        Error::Conflict(format!("invalid projection claim-set id: {error}"))
                    })?,
                    file_name,
                    schema_version,
                    knowledge_edge_count: usize::try_from(edges)
                        .map_err(|_| Error::Conflict("invalid projection edge count".to_owned()))?,
                    catalog_dependency_edge_count: usize::try_from(catalog_edges).map_err(
                        |_| Error::Conflict("invalid projection catalog count".to_owned()),
                    )?,
                    entity_fact_count: usize::try_from(facts)
                        .map_err(|_| Error::Conflict("invalid projection fact count".to_owned()))?,
                    content_digest: Digest::parse(&digest).map_err(|error| {
                        Error::Conflict(format!("invalid projection content digest: {error}"))
                    })?,
                })
            },
        )
        .transpose()
    }
}
