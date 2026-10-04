//! Kernel-controlled verification receipts for immutable graph projections.

use super::{
    build_types::{ProjectionReceipt, ProjectionReceiptIdentity},
    error::Error,
    projection_binding::{EXACT_RESOLVER_VERSION, InputMismatchKind, PROJECTION_REBUILD_REPAIR},
};
use crate::{
    artifact::Digest,
    job::{self, Lease, NewJob},
    scope::{Config, LOCAL, ScopeSet, collection_path},
    store::database::{HealthDatabase, HealthOpen, open_health_in},
    store::{self, Database},
};
use rusqlite::{OptionalExtension as _, Transaction, params};
use serde_json::json;
use std::{path::Path, time::SystemTime};

/// Stored projection-readiness receipt columns before decoding.
type ProjectionReceiptRow = (
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

/// Strict version-2 pins; legacy identities have no such tuple.
type ProjectionPins = (Digest, String, Digest, Digest);

/// Decode common fields before classifying the format or validating pins.
fn decode_identity(
    generation_id: i64,
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
    let identity = ProjectionReceiptIdentity {
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
    if identity.schema_version != "maestro-typed-edges/2"
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

/// Readiness admission remains strictly version 2, even for valid legacy metadata.
fn decode_receipt(
    generation_id: i64,
    row: ProjectionReceiptRow,
) -> Result<ProjectionReceipt, Error> {
    let (identity, pins) = decode_identity(generation_id, row)?;
    let (resolution_id, resolver_version, settings_identity, frozen_lock) =
        pins.ok_or(Error::ProjectionInputMismatch(InputMismatchKind::Format))?;
    Ok(ProjectionReceipt {
        identity,
        resolution_id,
        resolver_version,
        settings_identity,
        frozen_lock,
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
        now: SystemTime,
    ) -> Result<(), Error> {
        self.write(|transaction| {
            validate_publication(transaction, scopes, receipt, lease, now)?;
            let edge_count = i64::try_from(receipt.identity.knowledge_edge_count)
                .map_err(|_| Error::Conflict("projection edge count is too large".to_owned()))?;
            let catalog_count = i64::try_from(receipt.identity.catalog_dependency_edge_count)
                .map_err(|_| Error::Conflict("projection catalog count is too large".to_owned()))?;
            let fact_count = i64::try_from(receipt.identity.entity_fact_count)
                .map_err(|_| Error::Conflict("projection fact count is too large".to_owned()))?;
            transaction.execute(
                "INSERT INTO graph_projection_receipts
                 (generation_id, collection_id, claim_set_id, file_name, schema_version,
                  knowledge_edge_count, catalog_dependency_edge_count,
                  entity_fact_count, content_digest, resolution_id, resolver_version,
                  settings_identity, frozen_lock)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    receipt.identity.generation_id,
                    receipt.identity.collection_id,
                    receipt.identity.claim_set_id.as_str(),
                    receipt.identity.file_name,
                    receipt.identity.schema_version,
                    edge_count,
                    catalog_count,
                    fact_count,
                    receipt.identity.content_digest.as_str(),
                    receipt.resolution_id.as_str(),
                    receipt.resolver_version,
                    receipt.settings_identity.as_str(),
                    receipt.frozen_lock.as_str(),
                ],
            )?;
            Ok(())
        })
    }

    /// Check the exact project lease, expiry, attachment and receipt before native installation.
    /// Readiness recording repeats the same checks after installation; no native I/O runs here.
    ///
    /// # Errors
    /// Refuses expired/taken-over/cancelled leases, unauthorized or inconsistent receipts,
    /// existing readiness and kernel failures.
    pub fn validate_projection_publication(
        &self,
        scopes: &ScopeSet,
        receipt: &ProjectionReceipt,
        lease: &Lease,
        now: SystemTime,
    ) -> Result<(), Error> {
        self.write(|transaction| validate_publication(transaction, scopes, receipt, lease, now))
    }

    /// Check a scoped generation-specific project lease before starting or operating a producer.
    ///
    /// # Errors
    /// Refuses denied scopes, wrong jobs/generations, expired or lost leases and kernel failures.
    pub fn validate_projection_lease(
        &self,
        scopes: &ScopeSet,
        target: (&str, i64),
        lease: &Lease,
        now: SystemTime,
    ) -> Result<(), Error> {
        let scope = collection_path(target.0)
            .parse()
            .map_err(|_| Error::Unauthorized)?;
        if !scopes.covers(&scope) || target.1 <= 0 {
            return Err(Error::Unauthorized);
        }
        self.write(|transaction| validate_project_lease(transaction, target, lease, now))
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
        self.projection_receipt_row(scopes, generation)?
            .map(|row| decode_receipt(generation, row))
            .transpose()
    }

    /// Read strictly decoded file identity, including valid retained version-1 receipts.
    /// This authorizes cleanup only; it does not admit a native projection.
    ///
    /// # Errors
    /// Refuses malformed common fields, formats or input pins.
    pub fn projection_receipt_identity(
        &self,
        scopes: &ScopeSet,
        generation: i64,
    ) -> Result<Option<ProjectionReceiptIdentity>, Error> {
        self.projection_receipt_row(scopes, generation)?
            .map(|row| decode_identity(generation, row).map(|(identity, _)| identity))
            .transpose()
    }

    /// Query immutable receipt columns using the generation's visibility boundary.
    fn projection_receipt_row(
        &self,
        scopes: &ScopeSet,
        generation: i64,
    ) -> Result<Option<ProjectionReceiptRow>, Error> {
        self.reader()?
            .query_row(
                &format!(
                    "SELECT r.collection_id, r.claim_set_id, r.file_name, r.schema_version,
                            r.knowledge_edge_count, r.catalog_dependency_edge_count,
                            r.entity_fact_count, r.content_digest, r.resolution_id,
                            r.resolver_version,
                            r.settings_identity, r.frozen_lock
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
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                    ))
                },
            )
            .optional()
            .map_err(Error::from)
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
                    r.entity_fact_count, r.content_digest, r.resolution_id,
                            r.resolver_version,
                            r.settings_identity, r.frozen_lock
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
                        row.get(10)?,
                        row.get(11)?,
                        row.get(12)?,
                        row.get(13)?,
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
    inventory(data, principal, None)
}

/// Shared authority preflight and decoding for stored grants and read-only config policy.
fn inventory(
    data: &Path,
    principal: &str,
    config: Option<&Config>,
) -> Result<InventoryState, Error> {
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
            let scopes = match config {
                Some(config) => config.read_scopes(),
                None => kernel.visible(principal)?,
            };
            kernel
                .current_projection_inventory(&scopes)
                .map(InventoryState::Ready)
        }
    }
}

/// The one source of lease and authority checks before and after file installation.
fn validate_publication(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    receipt: &ProjectionReceipt,
    lease: &Lease,
    now: SystemTime,
) -> Result<(), Error> {
    if receipt.identity.generation_id <= 0
        || !receipt
            .identity
            .file_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        || !receipt
            .identity
            .file_name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || receipt.identity.schema_version != "maestro-typed-edges/2"
        || receipt.resolver_version != EXACT_RESOLVER_VERSION
    {
        return Err(Error::Conflict(format!(
            "invalid projection receipt identity; {PROJECTION_REBUILD_REPAIR}"
        )));
    }
    validate_project_lease(
        transaction,
        (
            &receipt.identity.collection_id,
            receipt.identity.generation_id,
        ),
        lease,
        now,
    )?;
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
                receipt.identity.generation_id,
                receipt.identity.collection_id,
                scopes.parameter()
            ],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((claim_set, edges, facts)) = expected else {
        return Err(Error::Unauthorized);
    };
    if claim_set != receipt.identity.claim_set_id.as_str()
        || usize::try_from(edges).ok() != Some(receipt.identity.knowledge_edge_count)
        || usize::try_from(facts).ok() != Some(receipt.identity.entity_fact_count)
    {
        return Err(Error::Conflict(
            "projection receipt differs from its authoritative claim set".to_owned(),
        ));
    }
    let already_ready: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM graph_projection_receipts WHERE generation_id = ?1)",
        [receipt.identity.generation_id],
        |row| row.get(0),
    )?;
    if already_ready {
        return Err(Error::Conflict(
            "projection readiness is already recorded".to_owned(),
        ));
    }
    Ok(())
}

/// Match the current live lease and frozen generation-specific project job.
fn validate_project_lease(
    transaction: &Transaction<'_>,
    target: (&str, i64),
    lease: &Lease,
    now: SystemTime,
) -> Result<(), Error> {
    let holder = job::validate_lease(transaction, lease)?;
    let timestamp = job::timestamp(transaction, Some(now))?;
    if holder
        .lease
        .as_ref()
        .is_none_or(|current| current.expires <= timestamp)
    {
        return Err(Error::Job(job::Error::Lost {
            job: lease.job,
            holder: lease.holder.clone(),
            number: lease.number,
        }));
    }
    let inputs = json!({"generation": target.1});
    let expected_job = NewJob {
        kind: "knowledge.graph.project",
        inputs: &inputs,
        scope: &holder.scope,
        resource: None,
    };
    if holder.scope.as_str() != collection_path(target.0)
        || holder.kind != expected_job.kind
        || holder.idempotency_key != job::idempotency_key(&expected_job)
    {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

/// Inventory using the local configuration without reconciling persistent grants.
///
/// # Errors
/// Refuses unreadable authority or malformed readiness records.
pub fn projection_inventory_with_config(
    data: &Path,
    config: &Config,
) -> Result<InventoryState, Error> {
    inventory(data, LOCAL, Some(config))
}
