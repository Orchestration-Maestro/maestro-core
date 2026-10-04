//! Scoped admission and lookup of immutable projection build reservations.
use super::{
    Error, ProjectionBuildRequest, ProjectionReceipt, ProjectionReceiptIdentity,
    ProjectionReservation, projection_records, projection_reservation,
};
use crate::{job::Lease, scope::ScopeSet, store::Database};
use std::time::SystemTime;
use ulid::Ulid;

impl Database {
    /// Reserve or replay exact frozen inputs under the current project lease.
    ///
    /// # Errors
    /// Refuses denied scopes, changed inputs, stale heads or invalid leases.
    pub fn begin_projection_build(
        &self,
        scopes: &ScopeSet,
        request: &ProjectionBuildRequest,
        lease: &Lease,
        now: SystemTime,
    ) -> Result<ProjectionReservation, Error> {
        self.write(|tx| projection_reservation::begin(tx, scopes, request, lease, now))
    }

    /// Look up an unfinished or historical reserved build; this grants no live authority.
    ///
    /// # Errors
    /// Refuses malformed stored inputs or unreadable storage.
    pub fn projection_build(
        &self,
        scopes: &ScopeSet,
        build: i64,
    ) -> Result<Option<ProjectionReservation>, Error> {
        self.write(|tx| projection_reservation::by_build(tx, scopes, build))
    }

    /// Look up the exact project attempt's reservation under current scopes.
    ///
    /// # Errors
    /// Refuses malformed stored inputs or unreadable storage.
    pub fn projection_build_for_job(
        &self,
        scopes: &ScopeSet,
        job: Ulid,
    ) -> Result<Option<ProjectionReservation>, Error> {
        self.write(|tx| projection_reservation::by_job(tx, scopes, job))
    }
}

impl Database {
    /// Read an exact scoped historical build's file identity, including legacy metadata.
    /// This does not select an active build or grant a live project lease.
    ///
    /// # Errors
    /// Refuses malformed receipt data or unreadable storage.
    pub fn projection_build_receipt_identity(
        &self,
        scopes: &ScopeSet,
        generation: i64,
        build: i64,
    ) -> Result<Option<ProjectionReceiptIdentity>, Error> {
        projection_records::by_build(&self.reader()?, scopes, generation, build)
            .map(|record| record.map(|record| record.identity))
    }

    /// Read the strictly pinned receipt of an explicitly selected scoped build.
    ///
    /// # Errors
    /// Refuses malformed metadata, unpinned legacy formats or unreadable storage.
    pub fn projection_build_receipt(
        &self,
        scopes: &ScopeSet,
        generation: i64,
        build: i64,
    ) -> Result<Option<ProjectionReceipt>, Error> {
        projection_records::by_build(&self.reader()?, scopes, generation, build)?
            .map(projection_records::Record::receipt)
            .transpose()
    }
}
