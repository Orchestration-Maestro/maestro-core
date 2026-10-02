//! Aggregate consumed capacity and one retained sync allocation across sequential sources.
use super::{controls::storage, resources};
use crate::failure::Failure;
use maestro_acquisition::{
    lifecycle::resources::{Reservation, Resources},
    policy::limits::Limits,
    transport::{budget::Usage, stream::Accounting},
};
use std::num::NonZeroU64;

/// One sync's consumed headroom and retained staging ownership across sources.
pub(crate) struct RunBudget {
    /// Eligible uncaptured attempts across all sources.
    pages: u64,
    /// New discovery and verification checkpoints across sources.
    partitions: u64,
    /// Every HTTP attempt, including robots, redirects and failures.
    requests: u64,
    /// Received response bytes across sources.
    wire: u64,
    /// Expanded bytes across sources.
    expanded: u64,
    /// All retained storage, including partial writes.
    pub(crate) staging: u64,
    /// Staging reservations survive until the whole sync stops.
    pub(crate) reservation: Option<Reservation>,
    /// Full OA3 aggregate envelope for the single owned reservation.
    pub(crate) bounds: [Limits; 1],
}
impl RunBudget {
    /// Deduct consumed aggregate capacity before source-local composition.
    pub(crate) fn remaining(&self, aggregate: &Limits) -> Option<Limits> {
        let mut limits = aggregate.clone();
        limits.pages = NonZeroU64::new(aggregate.pages.get().saturating_sub(self.pages))?;
        limits.partitions =
            NonZeroU64::new(aggregate.partitions.get().saturating_sub(self.partitions))?;
        limits.requests = NonZeroU64::new(aggregate.requests.get().saturating_sub(self.requests))?;
        limits.wire_bytes = NonZeroU64::new(aggregate.wire_bytes.get().saturating_sub(self.wire))?;
        limits.decode.expanded_bytes = NonZeroU64::new(
            aggregate
                .decode
                .expanded_bytes
                .get()
                .saturating_sub(self.expanded),
        )?;
        if self.staging >= self.bounds.first()?.staging_bytes.get() {
            return None;
        }
        Some(limits)
    }
    /// Reuse the one run slot, retaining old ownership even on a fresh-resource hold.
    pub(crate) fn reserve(&mut self, resources: &Resources, usage: Usage) -> Option<Reservation> {
        let owned = Usage {
            staging_bytes: self.staging,
            ..usage
        };
        if self.reservation.is_none() {
            self.reservation = resources.reserve(&self.bounds, owned).ok();
        }
        self.reservation.take().and_then(|mut reservation| {
            if reservation.checkpoint(&self.bounds, owned).is_ok() {
                Some(reservation)
            } else {
                self.reservation = Some(reservation);
                None
            }
        })
    }
    /// Account source-local observations without dropping aggregate storage ownership.
    pub(crate) fn record(
        &mut self,
        pages: u64,
        accounting: &Accounting,
        staging: u64,
        partitions: u64,
    ) -> Result<(), Failure> {
        self.partitions = self
            .partitions
            .checked_add(partitions)
            .ok_or_else(storage)?;
        self.pages = self.pages.checked_add(pages).ok_or_else(storage)?;
        self.requests = self
            .requests
            .checked_add(accounting.requests())
            .ok_or_else(storage)?;
        self.wire = self
            .wire
            .checked_add(accounting.wire_bytes())
            .ok_or_else(storage)?;
        self.expanded = self
            .expanded
            .checked_add(accounting.expanded_bytes())
            .ok_or_else(storage)?;
        self.staging = self.staging.checked_add(staging).ok_or_else(storage)?;
        Ok(())
    }
    /// Begin one aggregate run slot; sequential sources never retain extra slots.
    pub(crate) fn new(aggregate: &Limits) -> Result<Self, Failure> {
        let bounds = [resources::bounds(aggregate, false).map_err(|_| storage())?];
        Ok(Self {
            pages: 0,
            partitions: 0,
            requests: 0,
            wire: 0,
            expanded: 0,
            staging: 0,
            reservation: None,
            bounds,
        })
    }
}
