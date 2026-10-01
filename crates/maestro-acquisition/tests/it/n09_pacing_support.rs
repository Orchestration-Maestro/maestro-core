//! Recording fake that delegates N10 semantics while observing HTTP ownership.
use maestro_acquisition::{
    policy::identity::FetchIdentity,
    transport::pacing::{Demand, OriginLedger, OriginPacing, OriginPermit, PacingLimits, Pending},
};
use std::sync::{Arc, Mutex};

/// Every successful reservation and its corresponding owned release.
#[derive(Debug, Default)]
pub(super) struct Observations {
    pub(super) acquired: Vec<(String, u64, u64, u64)>,
    pub(super) active: usize,
    pub(super) released: usize,
}
/// Exact real ledger rules, with an explicit refusal for the zero-dial test.
#[derive(Debug)]
pub(super) struct RecordingLedger {
    pub(super) inner: OriginLedger,
    pub(super) observations: Arc<Mutex<Observations>>,
    pub(super) deny: bool,
    pub(super) deny_after: Option<usize>,
}
impl RecordingLedger {
    pub(super) fn new() -> Self {
        Self {
            inner: OriginLedger::new(0),
            observations: Arc::new(Mutex::new(Observations::default())),
            deny: false,
            deny_after: None,
        }
    }
}
impl OriginPacing for RecordingLedger {
    type Permit = RecordedPermit;
    fn acquire(
        &self,
        identity: &FetchIdentity,
        limits: PacingLimits,
        demand: Demand<'_>,
    ) -> Result<RecordedPermit, Pending> {
        if self.deny
            || self
                .deny_after
                .is_some_and(|count| self.observations.lock().unwrap().acquired.len() >= count)
        {
            return Err(Pending::Budget);
        }
        let permit = self.inner.acquire(identity, limits, demand)?;
        let mut observed = self.observations.lock().unwrap();
        // HTTP must release completed hop work before another reservation.
        assert_eq!(observed.active, 0);
        observed.active += 1;
        observed.acquired.push((
            identity.url().origin().ascii_serialization(),
            demand.retry,
            demand.now_ms,
            demand.server_delay_ms,
        ));
        Ok(RecordedPermit {
            _permit: permit,
            observations: self.observations.clone(),
        })
    }
}
/// Drops only after the connection/driver stops; never a cloned slot.
#[derive(Debug)]
pub(super) struct RecordedPermit {
    _permit: OriginPermit,
    observations: Arc<Mutex<Observations>>,
}
impl Drop for RecordedPermit {
    fn drop(&mut self) {
        let mut observed = self.observations.lock().unwrap();
        observed.active -= 1;
        observed.released += 1;
    }
}
