//! Kernel-controlled verification receipts for immutable graph projections.

use super::{
    build_types::{ProjectionReceipt, ProjectionReceiptIdentity},
    error::Error,
    projection_publication, projection_records, projection_reservation,
};
use crate::{
    job::Lease,
    scope::{Config, LOCAL, ScopeSet},
    store::database::{HealthDatabase, HealthOpen, open_health_in},
    store::{self, Database},
};
use std::{path::Path, time::SystemTime};

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
        self.write(|tx| projection_publication::publish(tx, scopes, receipt, lease, now))
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
        self.write(|tx| projection_publication::validate(tx, scopes, receipt, lease, now))
    }

    /// Check a scoped build-specific project lease before starting or operating a producer.
    ///
    /// # Errors
    /// Refuses denied scopes, wrong jobs/builds, expired or lost leases and kernel failures.
    pub fn validate_projection_lease(
        &self,
        scopes: &ScopeSet,
        target: (&str, i64),
        lease: &Lease,
        now: SystemTime,
    ) -> Result<(), Error> {
        self.write(|tx| {
            let reserved = projection_reservation::validate(tx, scopes, target.1, lease, now)?;
            if reserved.request.collection_id != target.0 {
                return Err(Error::Unauthorized);
            }
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
        self.active_projection(scopes, generation)?
            .map(projection_records::Record::receipt)
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
        Ok(self
            .active_projection(scopes, generation)?
            .map(|record| record.identity))
    }

    /// Select the exact active receipt using the shared joined authority read.
    fn active_projection(
        &self,
        scopes: &ScopeSet,
        generation: i64,
    ) -> Result<Option<projection_records::Record>, Error> {
        Ok(
            projection_records::active(&self.reader()?, scopes, Some(generation))?
                .into_iter()
                .next()
                .and_then(|(_, _, record)| record),
        )
    }
}

impl HealthDatabase {
    /// List published generations and their exact active receipt under current scopes.
    ///
    /// # Errors
    /// Refuses malformed or dangling active authority and unreadable storage.
    pub(crate) fn current_projection_inventory(
        &self,
        scopes: &ScopeSet,
    ) -> Result<Vec<ProjectionInventory>, Error> {
        projection_records::active(&self.connection, scopes, None)?
            .into_iter()
            .map(|(collection_id, generation_id, record)| {
                Ok(ProjectionInventory {
                    collection_id,
                    generation_id,
                    receipt: record
                        .map(projection_records::Record::receipt)
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
