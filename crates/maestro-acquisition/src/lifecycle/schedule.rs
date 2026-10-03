//! Local scheduling is a trigger, not an authority or a second acquisition queue.
use super::full::Mode;
use crate::{
    Refusal,
    policy::{
        format_time,
        source::{SyncMode, SyncPolicy},
    },
};
use maestro_kernel::{
    acquisition::Handle,
    job::{CREATED, Job, JobState, Lease, NewJob, stream},
    journal::{Filter, NewEvent},
    scope::{Scope, ScopeSet},
    store::Database,
};
use serde_json::json;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use ulid::Ulid;

/// OA3's explicitly enabled cadence floor; Pi has no acquisition cadence equivalent.
const MIN_CADENCE: Duration = Duration::from_hours(24);
/// Existing kernel job kind, independent of the source writer owned by sync.
pub const JOB_KIND: &str = "acquisition.schedule";
/// Durable stop request on the existing job journal, not a PID signal.
const STOP_REQUESTED: &str = "maestro.acquisition.schedule.stop_requested.v1";

/// How the identical admitted operation was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    /// Explicit caller action, never enabled by source content.
    Manual,
    /// An explicitly activated local timer or a future S4 adapter.
    Timer,
}
/// Fenced request shared by manual, local timer and future scheduler adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRequest {
    /// Exact explicit activation job, not a source permission.
    pub activation: Handle,
    /// Single-use sequence within that activation.
    pub sequence: u64,
    /// Full or incremental capture, unchanged downstream admission.
    pub mode: Mode,
    /// Policy-controlled request origin.
    pub kind: TriggerKind,
}
/// Replaceable adapter to the same admitted sync; implementations add no authority.
pub trait ScheduleTrigger {
    /// Invoke current admission and kernel source fencing before any effect.
    /// # Errors
    /// Current grants, policy, resources or source ownership can refuse.
    fn invoke(&mut self, request: &SyncRequest, principal: &str) -> Result<Handle, Refusal>;
}
/// One activation, permanently disabled by stop; restart requires another activation.
#[derive(Debug)]
pub struct Schedule {
    /// Exact single-use request identity.
    request: SyncRequest,
    /// Caller identity, never source-selected.
    principal: String,
    /// Declared scheduling mode.
    policy: SyncMode,
    /// Explicit checked watch cadence, absent for manual/one-off.
    cadence: Option<Duration>,
    /// Trusted monotonic next timer boundary.
    next: Instant,
    /// Stop or one-off consumption cannot be undone in this instance.
    active: bool,
}
impl Schedule {
    /// Bind strict policy with explicit OA3 cadence and caller identity.
    /// # Errors
    /// Missing/too-fast cadence, invalid principal or clock overflow refuses.
    pub fn new(
        policy: &SyncPolicy,
        activation: Handle,
        principal: &str,
        mode: Mode,
        now: Instant,
    ) -> Result<Self, Refusal> {
        if principal.is_empty() {
            return Err(Refusal::Invalid);
        }
        let cadence = if policy.mode == SyncMode::Watch {
            let period =
                Duration::from_millis(policy.timer_period_ms.ok_or(Refusal::Invalid)?.get());
            if period < MIN_CADENCE {
                return Err(Refusal::Invalid);
            }
            Some(period)
        } else {
            None
        };
        Ok(Self {
            request: SyncRequest {
                activation,
                sequence: 0,
                mode,
                kind: TriggerKind::Manual,
            },
            principal: principal.into(),
            policy: policy.mode,
            cadence,
            next: now
                .checked_add(cadence.unwrap_or(Duration::ZERO))
                .ok_or(Refusal::Invalid)?,
            active: true,
        })
    }
    /// Obtain the next single-use request, without dispatching anything.
    #[must_use]
    pub fn request(&self, kind: TriggerKind) -> SyncRequest {
        SyncRequest {
            kind,
            ..self.request.clone()
        }
    }
    /// Disable future triggers; this never alters acquisition pending state.
    pub fn stop(&mut self) {
        self.active = false;
    }
    /// Dispatch only current, due, policy-permitted requests; consume before invoking.
    /// # Errors
    /// Stopped, foreign, stale, duplicate or premature requests and admission refuse.
    pub fn invoke(
        &mut self,
        trigger: &mut dyn ScheduleTrigger,
        request: &SyncRequest,
        principal: &str,
        now: Instant,
    ) -> Result<Handle, Refusal> {
        if !self.active || principal != self.principal || *request != self.request(request.kind) {
            return Err(Refusal::Access);
        }
        if request.kind == TriggerKind::Timer
            && (self.policy == SyncMode::Manual || now < self.next)
        {
            return Err(Refusal::Access);
        }
        self.request.sequence = self
            .request
            .sequence
            .checked_add(1)
            .ok_or(Refusal::Invalid)?;
        if let Some(cadence) = self.cadence {
            // No catch-up burst: even a delayed trigger waits a full approved interval.
            self.next = now.checked_add(cadence).ok_or(Refusal::Invalid)?;
        }
        if self.policy == SyncMode::OneOff {
            self.stop();
        }
        trigger.invoke(request, principal)
    }
}
/// Explicit local activation inputs; there are no hidden lease or stop defaults.
#[derive(Debug)]
pub struct Activation<'a> {
    /// Checked source's scheduling policy.
    pub policy: &'a SyncPolicy,
    /// Authorized collection scope.
    pub scope: &'a Scope,
    /// Independently mapped local principal.
    pub principal: &'a str,
    /// The same lifecycle mode passed to manual sync.
    pub mode: Mode,
    /// Caller-supplied finite ownership term, renewed while the adapter is alive.
    pub lease_term: Duration,
}
/// Local timer adapter on the existing kernel job journal; owns no foreign child.
#[derive(Debug)]
pub struct LocalTimer<'a> {
    /// Existing kernel, also used by admitted sync.
    db: &'a Database,
    /// Read authorization; refreshed on every poll from the mapped principal.
    principal: String,
    /// Exclusive schedule ownership, separate from acquisition source writers.
    lease: Lease,
    /// Explicit caller term, with a monotonic expiry as well as durable expiry.
    term: Duration,
    /// Local deadline prevents wall-clock rollback reviving ownership.
    deadline: Instant,
    /// Pure scheduling controls, disabled before cancellation acknowledgement.
    schedule: Schedule,
}
impl<'a> LocalTimer<'a> {
    /// Explicitly create a new activation. This is not a live-source grant.
    /// Production composition must require an exact active-source grant first.
    /// # Errors
    /// Unauthorized scope, manual policy, invalid term, occupied resource or storage refuses.
    pub fn activate(
        db: &'a Database,
        activation: &Activation<'_>,
        now: Instant,
        wall: SystemTime,
    ) -> Result<Self, Refusal> {
        if activation.policy.mode == SyncMode::Manual || activation.lease_term.is_zero() {
            return Err(Refusal::Invalid);
        }
        let scopes = db
            .visible(activation.principal)
            .map_err(|_| Refusal::Access)?;
        if !scopes.covers(activation.scope) {
            return Err(Refusal::Access);
        }
        let deadline = now
            .checked_add(activation.lease_term)
            .ok_or(Refusal::Invalid)?;
        let identity = Handle::new();
        let mut schedule = Schedule::new(
            activation.policy,
            identity,
            activation.principal,
            activation.mode,
            now,
        )?;
        let resource = format!("acquisition/schedule/{}", activation.scope.as_str());
        let inputs = json!({
            "schema":"maestro-acquisition-schedule/1", "owner":activation.principal,
            "activation":identity, "sync":activation.policy, "mode":activation.mode
        });
        let job = db
            .submit_job(
                &NewJob {
                    kind: JOB_KIND,
                    inputs: &inputs,
                    scope: activation.scope,
                    resource: Some(&resource),
                },
                wall,
            )
            .map_err(|_| Refusal::Access)?;
        schedule.request.activation = job.id.to_string().parse().map_err(|_| Refusal::Invalid)?;
        let lease = db
            .take_job(job.id, activation.principal, wall, activation.lease_term)
            .map_err(|_| Refusal::Access)?;
        Ok(Self {
            db,
            principal: activation.principal.into(),
            lease,
            term: activation.lease_term,
            deadline,
            schedule,
        })
    }
    /// Exact activation job used by the owner-facing stop command.
    #[must_use]
    pub fn id(&self) -> Ulid {
        self.lease.job
    }
    /// Obtain the current request for another scheduling adapter.
    #[must_use]
    pub fn request(&self, kind: TriggerKind) -> SyncRequest {
        self.schedule.request(kind)
    }
    /// Recheck durable stop, current visibility and both ownership clocks before dispatch.
    /// # Errors
    /// Stop cancels the job and refuses; expired/taken-over ownership or admission refuses.
    pub fn invoke(
        &mut self,
        trigger: &mut dyn ScheduleTrigger,
        request: &SyncRequest,
        now: Instant,
        wall: SystemTime,
    ) -> Result<Handle, Refusal> {
        self.check(now, wall)?;
        self.dispatch(trigger, request, now)
    }
    /// Poll a real local timer; waiting is owned by its caller, not a detached scheduler.
    /// # Errors
    /// Same stop/lease/admission refusals as invocation; none implies completed capture.
    pub fn poll(
        &mut self,
        trigger: &mut dyn ScheduleTrigger,
        now: Instant,
        wall: SystemTime,
    ) -> Result<Option<Handle>, Refusal> {
        self.check(now, wall)?;
        if now < self.schedule.next {
            return Ok(None);
        }
        let request = self.request(TriggerKind::Timer);
        self.dispatch(trigger, &request, now).map(Some)
    }
    /// Dispatch a validated activation, consuming the one-off even on admitted failure.
    fn dispatch(
        &mut self,
        trigger: &mut dyn ScheduleTrigger,
        request: &SyncRequest,
        now: Instant,
    ) -> Result<Handle, Refusal> {
        let result = self.schedule.invoke(trigger, request, &self.principal, now);
        if self.schedule.policy == SyncMode::OneOff && !self.schedule.active {
            let state = if result.is_ok() {
                JobState::Succeeded
            } else {
                JobState::Failed
            };
            self.db
                .complete_job(&self.lease, state, &json!({"run":result.as_ref().ok()}))
                .map_err(|_| Refusal::Access)?;
        }
        result
    }
    /// The job journal is the single durable stop source, read before every trigger.
    fn check(&mut self, now: Instant, wall: SystemTime) -> Result<(), Refusal> {
        let scopes = self
            .db
            .visible(&self.principal)
            .map_err(|_| Refusal::Access)?;
        owned(self.db, &scopes, self.id(), &self.principal)?;
        if now >= self.deadline {
            self.schedule.stop();
            return Err(Refusal::Access);
        }
        if stop_requested(self.db, &scopes, self.id())? {
            self.schedule.stop();
            self.db
                .complete_job(
                    &self.lease,
                    JobState::Cancelled,
                    &json!({"reason":"owner_stop"}),
                )
                .map_err(|_| Refusal::Access)?;
            return Err(Refusal::Access);
        }
        let fraction = wall
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Refusal::Invalid)?
            .subsec_millis();
        let at = format!(
            "{}.{fraction:03}Z",
            format_time(wall)?.trim_end_matches('Z')
        );
        if at >= self.lease.expires {
            self.schedule.stop();
            return Err(Refusal::Access);
        }
        self.db
            .heartbeat(&mut self.lease, wall, self.term)
            .map_err(|_| Refusal::Access)?;
        self.deadline = now.checked_add(self.term).ok_or(Refusal::Invalid)?;
        Ok(())
    }
}
/// Verify the exact visible live schedule and its frozen local owner.
/// # Errors
/// Missing, terminal, wrong-kind, foreign-owner or denied jobs refuse.
fn owned(db: &Database, scopes: &ScopeSet, id: Ulid, principal: &str) -> Result<Job, Refusal> {
    let job = db
        .job(scopes, id)
        .map_err(|_| Refusal::Access)?
        .ok_or(Refusal::Access)?;
    if job.kind != JOB_KIND || !matches!(job.state, JobState::Queued | JobState::Running) {
        return Err(Refusal::Access);
    }
    let events = db
        .events(
            scopes,
            &Filter {
                stream: &stream(id),
                after: 0,
                r#type: Some(CREATED),
            },
        )
        .map_err(|_| Refusal::Access)?;
    if events
        .first()
        .and_then(|event| event.data.get("inputs")?.get("owner")?.as_str())
        != Some(principal)
    {
        return Err(Refusal::Access);
    }
    Ok(job)
}
/// Whether the durable owner stop request exists; no PID or process name is consulted.
/// # Errors
/// Unreadable authorized journal refuses.
pub fn stop_requested(db: &Database, scopes: &ScopeSet, id: Ulid) -> Result<bool, Refusal> {
    Ok(!db
        .events(
            scopes,
            &Filter {
                stream: &stream(id),
                after: 0,
                r#type: Some(STOP_REQUESTED),
            },
        )
        .map_err(|_| Refusal::Access)?
        .is_empty())
}
/// Disable a schedule durably. The owner waits separately for honest cancellation acknowledgement.
/// # Errors
/// Foreign, unavailable, terminal or mismatched jobs and storage failures refuse.
pub fn request_stop(
    db: &Database,
    scopes: &ScopeSet,
    id: Ulid,
    principal: &str,
) -> Result<(), Refusal> {
    let job = owned(db, scopes, id, principal)?;
    if !stop_requested(db, scopes, id)? {
        db.record(&NewEvent {
            stream: &stream(id),
            r#type: STOP_REQUESTED,
            subject: &stream(id),
            scope: job.scope.as_str(),
            data: &json!({"owner":principal}),
        })
        .map_err(|_| Refusal::Access)?;
    }
    if job.state == JobState::Queued {
        db.cancel_job(id, &json!({"reason":"owner_stop"}))
            .map_err(|_| Refusal::Access)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Schedule, ScheduleTrigger, SyncRequest, TriggerKind};
    use crate::{
        Refusal,
        lifecycle::full::Mode,
        policy::source::{SyncMode, SyncPolicy},
    };
    use maestro_kernel::acquisition::Handle;
    use std::time::Instant;

    /// Overflow must refuse before invoking any admitted operation.
    struct Unreachable;
    impl ScheduleTrigger for Unreachable {
        fn invoke(&mut self, _: &SyncRequest, _: &str) -> Result<Handle, Refusal> {
            panic!("sequence overflow dispatched");
        }
    }
    #[test]
    fn n42_sequence_overflow_refuses_before_dispatch() {
        let policy = SyncPolicy {
            mode: SyncMode::Manual,
            timer_period_ms: None,
            overlap_ms: 0,
            clock_skew_ms: 0,
            revision_fields: vec![],
        };
        let now = Instant::now();
        let mut schedule = Schedule::new(&policy, Handle::new(), "owner", Mode::Full, now).unwrap();
        schedule.request.sequence = u64::MAX;
        let request = schedule.request(TriggerKind::Manual);
        assert!(
            schedule
                .invoke(&mut Unreachable, &request, "owner", now)
                .is_err()
        );
    }
}
