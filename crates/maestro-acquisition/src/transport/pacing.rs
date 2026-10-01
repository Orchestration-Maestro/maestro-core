//! One injected atomic origin ledger shared by HTTP, browser and resumed runs.
//!
//! Ceilings compose with min; interval/server-delay floors with max. A newly
//! created ledger treats every origin as just accessed: restarts slow down,
//! never accelerate dispatch. Host monotonic times and run IDs are trusted inputs.
use crate::{
    Refusal,
    policy::{identity::FetchIdentity, limits::Limits},
};
use std::{
    collections::BTreeMap,
    fmt::Debug,
    sync::{Arc, Mutex},
    time::Instant,
};

/// Finite pacing bounds resolved from host/grant/collection/source/run policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacingLimits {
    /// At-least request spacing, composed with max.
    interval_ms: u64,
    /// At-most concurrent requests, composed with min.
    concurrency: u64,
    /// At-most attempts for a logical run across all origins.
    requests: u64,
    /// At-most retries per request, including zero.
    retries: u64,
    /// At-most retry wait; a longer required delay remains pending.
    max_backoff_ms: u64,
    /// At-most elapsed duration of the logical run.
    elapsed_ms: u64,
}
impl PacingLimits {
    /// All supplied levels constrain effects; an absent envelope refuses.
    ///
    /// # Errors
    /// No finite supplied limits means no dispatch.
    pub fn compose<'a>(levels: impl IntoIterator<Item = &'a Limits>) -> Result<Self, Refusal> {
        let mut levels = levels.into_iter().map(Self::from_limits);
        let mut effective = levels.next().ok_or(Refusal::Missing)?;
        for next in levels {
            effective = effective.tighten(next);
        }
        Ok(effective)
    }
    /// Read the effective max-composed interval floor.
    #[must_use]
    pub fn interval_ms(self) -> u64 {
        self.interval_ms
    }
    /// Read the effective min-composed concurrency ceiling.
    #[must_use]
    pub fn concurrency(self) -> u64 {
        self.concurrency
    }
    /// Extract only N10's enforced field kinds from the existing strict schema.
    fn from_limits(limits: &Limits) -> Self {
        Self {
            interval_ms: limits.origin_interval_ms.get(),
            concurrency: limits.origin_concurrency.get(),
            requests: limits.requests.get(),
            retries: limits.retries,
            max_backoff_ms: limits.max_backoff_ms,
            elapsed_ms: limits.elapsed_ms.get(),
        }
    }
    /// Compose all fields, also used on live tightening and resume.
    fn tighten(self, next: Self) -> Self {
        Self {
            interval_ms: self.interval_ms.max(next.interval_ms),
            concurrency: self.concurrency.min(next.concurrency),
            requests: self.requests.min(next.requests),
            retries: self.retries.min(next.retries),
            max_backoff_ms: self.max_backoff_ms.min(next.max_backoff_ms),
            elapsed_ms: self.elapsed_ms.min(next.elapsed_ms),
        }
    }
}
/// Current request attempt supplied by the owning run/frontier, never content.
#[derive(Debug, Clone, Copy)]
pub struct Demand<'a> {
    /// Logical run identity survives pause/resume; different runs remain distinct.
    pub run_id: &'a str,
    /// Original logical run start in the host monotonic domain.
    pub started_ms: u64,
    /// Current host monotonic time.
    pub now_ms: u64,
    /// Finite owning-work deadline; elapsed policy may impose a sooner deadline.
    pub deadline_ms: u64,
    /// Retry number; zero denotes the initial attempt.
    pub retry: u64,
    /// Applicable remaining server delay, already parsed by HTTP/browser adapter.
    /// Both Retry-After forms must be translated without shortening this floor.
    pub server_delay_ms: u64,
}
impl<'a> Demand<'a> {
    /// A non-retry request with no known server delay.
    #[must_use]
    pub fn initial(run_id: &'a str, started_ms: u64, now_ms: u64, deadline_ms: u64) -> Self {
        Self {
            run_id,
            started_ms,
            now_ms,
            deadline_ms,
            retry: 0,
            server_delay_ms: 0,
        }
    }
}
/// Dispatch did not happen; retain the work pending with this fixed reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// No permit until the full origin/server floor has elapsed.
    Delay {
        /// Earliest host monotonic dispatch time; never a shortened wait.
        until_ms: u64,
    },
    /// Existing requests occupy the effective origin concurrency ceiling.
    Concurrency,
    /// Requests/retries/backoff/elapsed exhausted, invalid clock or unavailable ledger.
    Budget,
}
/// One shared origin's state, independent of source and transport kind.
#[derive(Debug)]
struct OriginState {
    /// Stricter prior bounds cannot be loosened by another run or resume.
    limits: PacingLimits,
    /// Latest dispatch time, seeded from ledger startup for unknown origins.
    last_ms: u64,
    /// Server delay applies even when the observing run cannot afford to wait.
    server_until_ms: u64,
    /// Owned active permits, released only when owned work stops.
    active: u64,
}
/// Shared attempt accounting for a logical run across different origins.
#[derive(Debug)]
struct RunState {
    /// Tightest observed run bounds.
    limits: PacingLimits,
    /// Attempt count includes retries but excludes denied/pending requests.
    attempts: u64,
    /// Original start time cannot change on resume.
    started_ms: u64,
}
/// Replaceable reservation seam; every successful permit releases on drop.
pub trait OriginPacing: Debug {
    /// Owned slot retained until the transport's work has stopped.
    type Permit;
    /// Reserve one actual destination dispatch using N10's finite contract.
    ///
    /// # Errors
    /// Pending work must make no dial; delay floors must never be shortened.
    fn acquire(
        &self,
        identity: &FetchIdentity,
        limits: PacingLimits,
        demand: Demand<'_>,
    ) -> Result<Self::Permit, Pending>;
}
/// Trusted logical-run context; never supplied by a remote response.
#[derive(Debug, Clone, Copy)]
pub struct PacingContext<'a> {
    /// Stable identity shared across transports and pause/resume.
    pub run_id: &'a str,
    /// Epoch shared with the injected ledger and document clock.
    pub epoch: Instant,
    /// Original logical-run start, relative to the shared epoch.
    pub started_ms: u64,
    /// Owning logical-run deadline, relative to the shared epoch.
    pub deadline_ms: u64,
}
/// State guarded by one short mutex; no network/sleep under the lock.
#[derive(Debug, Default)]
struct State {
    /// Exact HTTPS origin keys include nondefault ports.
    origins: BTreeMap<String, OriginState>,
    /// Logical run counters outlive a dropped individual permit.
    runs: BTreeMap<String, RunState>,
}
/// Replaceable transport callers receive clones of this one injected ledger.
#[derive(Debug, Clone)]
pub struct OriginLedger {
    /// Trusted startup time; unknown origins are treated as just accessed.
    started_ms: u64,
    /// One atomic admission critical section spans all transport callers.
    state: Arc<Mutex<State>>,
}
impl OriginLedger {
    /// Start conservatively after process restart; retain this object on resume.
    #[must_use]
    pub fn new(now_ms: u64) -> Self {
        Self {
            started_ms: now_ms,
            state: Arc::new(Mutex::new(State::default())),
        }
    }
    /// Reserve before any transport request, including rules refresh and retries.
    /// A denied robots result must never call this to dispatch its destination.
    /// Keep the permit until the underlying owned work has actually stopped.
    ///
    /// # Errors
    /// Concurrency, interval/server delay or finite budgets keep work pending.
    /// Required waits beyond backoff/deadline are never shortened.
    #[expect(
        clippy::same_name_method,
        reason = "preserve N10 inherent API and exact reservation trait contract"
    )]
    pub fn acquire(
        &self,
        identity: &FetchIdentity,
        limits: PacingLimits,
        demand: Demand<'_>,
    ) -> Result<OriginPermit, Pending> {
        let origin = identity.url().origin().ascii_serialization();
        let mut state = self.state.lock().map_err(|_| Pending::Budget)?;
        let State { origins, runs } = &mut *state;
        let entry = origins.entry(origin.clone()).or_insert(OriginState {
            limits,
            last_ms: self.started_ms,
            server_until_ms: self.started_ms,
            active: 0,
        });
        entry.limits = entry.limits.tighten(limits);
        let run = runs.entry(demand.run_id.to_owned()).or_insert(RunState {
            limits,
            attempts: 0,
            started_ms: demand.started_ms,
        });
        run.limits = run.limits.tighten(limits);
        let deadline = demand.deadline_ms.min(
            run.started_ms
                .checked_add(run.limits.elapsed_ms)
                .ok_or(Pending::Budget)?,
        );
        let server_until = demand
            .now_ms
            .checked_add(demand.server_delay_ms)
            .ok_or(Pending::Budget)?;
        entry.server_until_ms = entry.server_until_ms.max(server_until);
        if demand.run_id.is_empty()
            || demand.started_ms != run.started_ms
            || demand.now_ms < entry.last_ms
            || demand.now_ms < run.started_ms
            || demand.retry > run.limits.retries
        {
            return Err(Pending::Budget);
        }
        if entry.active >= entry.limits.concurrency {
            return Err(Pending::Concurrency);
        }
        if run.attempts >= run.limits.requests {
            return Err(Pending::Budget);
        }
        let until_ms = entry
            .last_ms
            .checked_add(entry.limits.interval_ms)
            .ok_or(Pending::Budget)?
            .max(entry.server_until_ms);
        let wait = until_ms.saturating_sub(demand.now_ms);
        if until_ms >= deadline || wait > run.limits.max_backoff_ms {
            return Err(Pending::Budget);
        }
        if demand.now_ms < until_ms {
            return Err(Pending::Delay { until_ms });
        }
        entry.active += 1;
        entry.last_ms = demand.now_ms;
        run.attempts += 1;
        Ok(OriginPermit {
            origin,
            state: Arc::clone(&self.state),
        })
    }
}
impl OriginPacing for OriginLedger {
    type Permit = OriginPermit;
    fn acquire(
        &self,
        identity: &FetchIdentity,
        limits: PacingLimits,
        demand: Demand<'_>,
    ) -> Result<Self::Permit, Pending> {
        Self::acquire(self, identity, limits, demand)
    }
}
/// An owned dispatch slot, not Clone; dropping it releases concurrency only.
/// Never drop merely because an HTTP timeout returned while work is still alive.
#[derive(Debug)]
pub struct OriginPermit {
    /// Exact origin whose active count this permit owns.
    origin: String,
    /// Shared ledger outlives this run/transport handle.
    state: Arc<Mutex<State>>,
}
impl Drop for OriginPermit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock()
            && let Some(entry) = state.origins.get_mut(&self.origin)
        {
            entry.active -= 1;
        }
    }
}
