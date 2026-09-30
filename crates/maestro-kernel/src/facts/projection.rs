//! Kernel-controlled verification receipts for immutable graph projections.

use super::{build_types::ProjectionReceipt, error::Error};
use crate::{
    artifact::Digest,
    job::{self, Lease, NewJob},
    scope::{ScopeSet, collection_path},
    store::database::{HealthDatabase, HealthOpen, open_health_in},
    store::{self, Database},
};
use rusqlite::{OptionalExtension as _, params};
use serde_json::json;
use std::path::Path;

/// Stored projection-readiness receipt columns before decoding.
type ProjectionReceiptRow = (String, String, String, String, i64, i64, i64, String);

/// A current published generation and its readiness receipt, if present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionInventory {
    /// Collection owning this current generation.
    pub collection_id: String,
    /// Published generation identifier.
    pub generation_id: i64,
    /// Exact stored readiness receipt, absent when the projection is not ready.
    pub receipt: Option<ProjectionReceipt>,
}

/// Decode the stored columns with the same validation used by normal receipt lookup.
fn decode_receipt(
    generation_id: i64,
    row: ProjectionReceiptRow,
) -> Result<ProjectionReceipt, Error> {
    let (collection_id, set, file_name, schema_version, edges, catalog_edges, facts, digest) = row;
    Ok(ProjectionReceipt {
        collection_id,
        generation_id,
        claim_set_id: Digest::parse(&set).map_err(|error| {
            Error::Conflict(format!("invalid projection claim-set id: {error}"))
        })?,
        file_name,
        schema_version,
        knowledge_edge_count: usize::try_from(edges)
            .map_err(|_| Error::Conflict("invalid projection edge count".to_owned()))?,
        catalog_dependency_edge_count: usize::try_from(catalog_edges)
            .map_err(|_| Error::Conflict("invalid projection catalog count".to_owned()))?,
        entity_fact_count: usize::try_from(facts)
            .map_err(|_| Error::Conflict("invalid projection fact count".to_owned()))?,
        content_digest: Digest::parse(&digest).map_err(|error| {
            Error::Conflict(format!("invalid projection content digest: {error}"))
        })?,
    })
}

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
        row.map(|row| decode_receipt(generation, row)).transpose()
    }
}

impl HealthDatabase {
    /// List current published generations visible to `scopes`, in collection and generation order.
    /// Retired receipts are omitted; a current generation without readiness remains visible.
    ///
    /// # Errors
    /// Returns a facts error when generation or receipt data is unreadable or malformed.
    pub(crate) fn current_projection_inventory(
        &self,
        scopes: &ScopeSet,
    ) -> Result<Vec<ProjectionInventory>, Error> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT g.collection_id, g.id, r.collection_id, r.claim_set_id, r.file_name,
                    r.schema_version, r.knowledge_edge_count, r.catalog_dependency_edge_count,
                    r.entity_fact_count, r.content_digest
             FROM generations g LEFT JOIN graph_projection_receipts r ON r.generation_id = g.id
             WHERE g.state = 'published' AND {}
             ORDER BY g.collection_id, g.id",
            ScopeSet::collection_condition("g.collection_id", 1)
        ))?;
        let rows = statement.query_map([scopes.parameter()], |row| {
            let receipt = row
                .get::<_, Option<String>>(2)?
                .map(|collection_id| {
                    Ok::<_, rusqlite::Error>((
                        collection_id,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                    ))
                })
                .transpose()?;
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, receipt))
        })?;
        rows.map(|row| {
            let (collection_id, generation_id, receipt) = row?;
            Ok(ProjectionInventory {
                collection_id,
                generation_id,
                receipt: receipt
                    .map(|receipt| decode_receipt(generation_id, receipt))
                    .transpose()?,
            })
        })
        .collect()
    }
}

/// Read-only status and receipt inventory for the kernel's graph projections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryState {
    /// No kernel database exists.
    Missing,
    /// The database lacks migrations required by this binary.
    NeedsMigration(Vec<&'static str>),
    /// The database records a migration this binary does not carry.
    NewerSchema(String),
    /// Current published generations visible to the principal.
    Ready(Vec<ProjectionInventory>),
}

/// Open a kernel read-only and list the graph projection receipts visible to `principal`.
///
/// # Errors
/// Returns a facts error wrapping store failures or malformed receipt data.
pub fn projection_inventory_in(data: &Path, principal: &str) -> Result<InventoryState, Error> {
    let health = match open_health_in(data) {
        Ok(health) => health,
        Err(store::Error::UnknownMigration(name)) => {
            return Ok(InventoryState::NewerSchema(name));
        }
        Err(error) => return Err(error.into()),
    };
    match health {
        HealthOpen::Missing => Ok(InventoryState::Missing),
        HealthOpen::NeedsMigration(names) => Ok(InventoryState::NeedsMigration(names)),
        HealthOpen::Ready(kernel) => {
            let scopes = kernel.visible(principal)?;
            kernel
                .current_projection_inventory(&scopes)
                .map(InventoryState::Ready)
        }
    }
}
