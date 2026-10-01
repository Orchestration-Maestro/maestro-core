//! Shared owned reservations, with fresh measurements at every checkpoint.
use crate::{
    policy::limits::Limits,
    transport::budget::{Pending, Usage, compose, tighten},
};
use std::{
    collections::BTreeMap,
    fmt::Debug,
    sync::{Arc, Mutex, PoisonError},
};

/// Fresh enforceable host/grant settings and measurements, never policy authority.
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    /// Current aggregate host/grant/collection envelope.
    pub aggregate: Limits,
    /// Current host/grant per-run envelope.
    pub per_run: Limits,
    /// Currently measured free backing-store bytes.
    pub free_disk_bytes: u64,
    /// Currently measured free VRAM; loaded interactive models remain untouched.
    pub free_gpu_bytes: u64,
    /// Interactive demand takes priority over ingestion at safe checkpoints.
    pub interactive_pending: bool,
}

/// Replaceable settings/measurement boundary, also usable by the S4 scheduler.
pub trait ResourceControls: Debug + Send + Sync {
    /// Read fresh host/grant bounds and measurements under the ledger lock.
    ///
    /// Implementations must not call back into this ledger. Unknown measurements,
    /// absent grants and unsupported enforcement fail closed; no live authorization
    /// or GPU scheduling is created here. This port does not replace gpuq.
    ///
    /// # Errors
    /// Return a typed hold when authority, measurements or enforcement are absent.
    fn snapshot(&self) -> Result<ResourceSnapshot, Pending>;
}

/// Safe default until a current authority adapter is bound; grants are not guessed.
#[derive(Debug)]
pub struct NoGrantControls;
impl ResourceControls for NoGrantControls {
    fn snapshot(&self) -> Result<ResourceSnapshot, Pending> {
        Err(Pending::Grant)
    }
}

/// One run's retained tighter bounds, owned allocations and current pending reason.
#[derive(Debug)]
struct Run {
    /// Bounds cannot loosen on resume.
    limits: Limits,
    /// Allocations retained until the owner stops and releases them.
    usage: Usage,
    /// Latest checkpoint hold, or none after a successful checkpoint.
    pending: Option<Pending>,
}
/// Mutable ownership shared by all clones of one ledger.
#[derive(Debug, Default)]
struct State {
    /// Never reused process-local ownership identifiers.
    next: u64,
    /// Retained aggregate tightening; no unbounded initial default.
    aggregate: Option<Limits>,
    /// Active owned reservations.
    runs: BTreeMap<u64, Run>,
}
/// Share one ledger across all sources; restarting creates no stale headroom.
///
/// Disk admission conservatively subtracts all owned staging bytes from fresh
/// measured free space, even when a completed write is already reflected there.
/// This may pause early, but never weakens the reserve. Acknowledged write/measure
/// accounting belongs with the first live writer, not this process-local ledger.
#[derive(Debug, Clone)]
pub struct Resources {
    /// Consumer-bound settings and measurements.
    controls: Arc<dyn ResourceControls>,
    /// Atomic combined admission and ownership.
    // ponytail: process-local lock; kernel-backed accounting for multi-process use.
    state: Arc<Mutex<State>>,
}
impl Resources {
    /// Bind a replaceable control port; no grant, process or GPU job is created.
    #[must_use]
    pub fn new(controls: Arc<dyn ResourceControls>) -> Self {
        Self {
            controls,
            state: Arc::new(Mutex::new(State::default())),
        }
    }
    /// Reserve owned allocations before starting a run.
    ///
    /// Bounds must include the complete reviewed source/run envelope. The control
    /// port supplies current host/grant bounds; neither alone grants live access.
    ///
    /// # Errors
    /// Missing or incompatible controls and exceeded ceilings leave work pending.
    pub fn reserve(&self, bounds: &[Limits], usage: Usage) -> Result<Reservation, Pending> {
        let mut state = self.state.lock().map_err(|_| Pending::Accounting)?;
        let snapshot = self.controls.snapshot()?;
        let mut limits = compose(bounds)?;
        effective(&mut state, &snapshot, &mut limits);
        check(&state, &snapshot, &limits, usage, None)?;
        let id = state.next;
        state.next = state.next.checked_add(1).ok_or(Pending::Accounting)?;
        state.runs.insert(
            id,
            Run {
                limits,
                usage,
                pending: None,
            },
        );
        Ok(Reservation {
            resources: self.clone(),
            id,
        })
    }
    /// Expose combined owned usage, not a fabricated host measurement.
    ///
    /// # Errors
    /// Poisoned state or arithmetic overflow holds accounting.
    pub fn usage(&self) -> Result<Usage, Pending> {
        let state = self.state.lock().map_err(|_| Pending::Accounting)?;
        state
            .runs
            .values()
            .try_fold(Usage::default(), |sum, run| add(sum, run.usage))
    }
}
/// RAII ownership, not a process supervisor or cancellation acknowledgment.
///
/// Before dropping, the caller must stop/reap its owned work and clean staging.
/// Drop releases only ledger ownership, never another run's allocations or models.
#[derive(Debug)]
pub struct Reservation {
    /// Shared accounting owner.
    resources: Resources,
    /// Exact owned run, never supplied by an untrusted source.
    id: u64,
}
impl Reservation {
    /// Recheck fresh controls and tighter bounds before streaming/resuming work.
    ///
    /// The supplied usage is the replacement total, not an increment. On a hold,
    /// old allocations remain owned until the caller stops/releases them. Interactive
    /// demand pauses ingestion here, not by evicting a model or cancelling a job.
    ///
    /// # Errors
    /// Incompatible bounds or unavailable controls checkpoint/pause ingestion.
    pub fn checkpoint(&mut self, bounds: &[Limits], usage: Usage) -> Result<(), Pending> {
        let mut state = self
            .resources
            .state
            .lock()
            .map_err(|_| Pending::Accounting)?;
        let result = update(
            &mut state,
            self.resources.controls.as_ref(),
            self.id,
            bounds,
            usage,
        );
        state
            .runs
            .get_mut(&self.id)
            .ok_or(Pending::Accounting)?
            .pending = result.err();
        result
    }
    /// Inspect this run's most recent checkpoint hold.
    ///
    /// # Errors
    /// Missing ownership or poisoned accounting state holds.
    pub fn pending(&self) -> Result<Option<Pending>, Pending> {
        let state = self
            .resources
            .state
            .lock()
            .map_err(|_| Pending::Accounting)?;
        Ok(state.runs.get(&self.id).ok_or(Pending::Accounting)?.pending)
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut state = self
            .resources
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        state.runs.remove(&self.id);
    }
}

/// Preserve tighter per-run and aggregate controls even when allocation holds.
fn effective(state: &mut State, snapshot: &ResourceSnapshot, limits: &mut Limits) {
    let aggregate = state
        .aggregate
        .get_or_insert_with(|| snapshot.aggregate.clone());
    tighten(aggregate, &snapshot.aggregate);
    tighten(limits, aggregate);
    tighten(limits, &snapshot.per_run);
}

/// Validate replacement usage without losing the previous owned allocations.
fn update(
    state: &mut State,
    controls: &dyn ResourceControls,
    id: u64,
    bounds: &[Limits],
    usage: Usage,
) -> Result<(), Pending> {
    let snapshot = controls.snapshot()?;
    let mut limits = compose(bounds)?;
    tighten(
        &mut limits,
        &state.runs.get(&id).ok_or(Pending::Accounting)?.limits,
    );
    effective(state, &snapshot, &mut limits);
    state.runs.get_mut(&id).ok_or(Pending::Accounting)?.limits = limits.clone();
    check(state, &snapshot, &limits, usage, Some(id))?;
    state.runs.get_mut(&id).ok_or(Pending::Accounting)?.usage = usage;
    Ok(())
}

/// Check every allocation atomically, with all active reserve floors retained.
fn check(
    state: &State,
    snapshot: &ResourceSnapshot,
    limits: &Limits,
    usage: Usage,
    replacing: Option<u64>,
) -> Result<(), Pending> {
    if snapshot.interactive_pending {
        return Err(Pending::Interactive);
    }
    check_ceilings(usage, limits)?;
    let aggregate = state.aggregate.as_ref().ok_or(Pending::Bounds)?;
    let mut combined = usage;
    let mut disk_reserve = limits.free_reserve_bytes.get();
    let mut gpu_reserve = limits.gpu_reserve_bytes.get();
    for (&id, run) in &state.runs {
        if Some(id) != replacing {
            combined = add(combined, run.usage)?;
            disk_reserve = disk_reserve.max(run.limits.free_reserve_bytes.get());
            gpu_reserve = gpu_reserve.max(run.limits.gpu_reserve_bytes.get());
        }
    }
    check_ceilings(combined, aggregate)?;
    let count = u64::try_from(state.runs.len()).map_err(|_| Pending::Accounting)?;
    let count = count
        .checked_add(u64::from(replacing.is_none()))
        .ok_or(Pending::Accounting)?;
    if count > aggregate.source_runs.get() {
        return Err(Pending::Runs);
    }
    if snapshot.free_disk_bytes < disk_reserve
        || combined.staging_bytes > snapshot.free_disk_bytes.saturating_sub(disk_reserve)
    {
        return Err(Pending::DiskReserve);
    }
    let gpu_headroom = snapshot
        .free_gpu_bytes
        .saturating_sub(gpu_reserve)
        .min(aggregate.gpu_bytes);
    if combined.gpu_bytes > gpu_headroom || (combined.gpu_batches > 0 && gpu_headroom == 0) {
        return Err(Pending::Gpu);
    }
    Ok(())
}

/// One common per-run/aggregate ceiling check; zero GPU means disabled.
fn check_ceilings(usage: Usage, limits: &Limits) -> Result<(), Pending> {
    for (amount, ceiling, reason) in [
        (
            usage.cpu_millicores,
            limits.cpu_millicores.get(),
            Pending::Cpu,
        ),
        (
            usage.memory_bytes,
            limits.memory_bytes.get(),
            Pending::Memory,
        ),
        (
            usage.staging_bytes,
            limits.staging_bytes.get(),
            Pending::Staging,
        ),
        (usage.gpu_bytes, limits.gpu_bytes, Pending::Gpu),
        (usage.gpu_batches, limits.gpu_batches, Pending::GpuBatch),
    ] {
        if amount > ceiling {
            return Err(reason);
        }
    }
    if usage.gpu_batches > 0 && limits.gpu_bytes == 0 {
        return Err(Pending::Gpu);
    }
    Ok(())
}

/// Checked combined accounting never wraps on adversarial allocation counts.
fn add(left: Usage, right: Usage) -> Result<Usage, Pending> {
    Ok(Usage {
        cpu_millicores: left
            .cpu_millicores
            .checked_add(right.cpu_millicores)
            .ok_or(Pending::Accounting)?,
        memory_bytes: left
            .memory_bytes
            .checked_add(right.memory_bytes)
            .ok_or(Pending::Accounting)?,
        staging_bytes: left
            .staging_bytes
            .checked_add(right.staging_bytes)
            .ok_or(Pending::Accounting)?,
        gpu_bytes: left
            .gpu_bytes
            .checked_add(right.gpu_bytes)
            .ok_or(Pending::Accounting)?,
        gpu_batches: left
            .gpu_batches
            .checked_add(right.gpu_batches)
            .ok_or(Pending::Accounting)?,
    })
}
