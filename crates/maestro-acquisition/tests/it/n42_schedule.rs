//! N42 scheduling guards on injected clocks and existing kernel jobs.
use maestro_acquisition::lifecycle::schedule::{
    Activation, LocalTimer, Schedule, ScheduleTrigger, SyncRequest, TriggerKind, request_stop,
    stop_requested,
};
use maestro_acquisition::{
    Refusal,
    lifecycle::full::Mode,
    policy::source::{SyncMode, SyncPolicy},
};
use maestro_kernel::{
    acquisition::Handle,
    job::{JobState, NewJob, stream},
    journal::Filter,
    scope::Right,
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use serde_json::json;
use std::{
    fs,
    num::NonZeroU64,
    time::{Duration, Instant, UNIX_EPOCH},
};
use ulid::Ulid;

/// Typed admission refusal, never a swallowed scheduling success.
struct Refusing;
impl ScheduleTrigger for Refusing {
    fn invoke(&mut self, _: &SyncRequest, _: &str) -> Result<Handle, Refusal> {
        Err(Refusal::Access)
    }
}

#[test]
fn n42_owned_timer_fences_expiry_and_preserves_one_off_failure() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    let policy = SyncPolicy {
        mode: SyncMode::OneOff,
        timer_period_ms: None,
        overlap_ms: 0,
        clock_skew_ms: 0,
        revision_fields: vec![],
    };
    let activation = Activation {
        policy: &policy,
        scope: &scope,
        principal: "owner",
        mode: Mode::Full,
        lease_term: Duration::from_secs(10),
    };
    let now = Instant::now();
    let wall = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let mut timer = LocalTimer::activate(&db, &activation, now, wall).unwrap();
    let mut stale = timer.request(TriggerKind::Timer);
    stale.activation = Handle::new();
    assert!(timer.invoke(&mut Refusing, &stale, now, wall).is_err());
    let scopes = db.visible("owner").unwrap();
    assert_eq!(
        db.job(&scopes, timer.id()).unwrap().unwrap().state,
        JobState::Running
    );
    assert!(timer.poll(&mut Refusing, now, wall).is_err());
    assert_eq!(
        db.job(&scopes, timer.id()).unwrap().unwrap().state,
        JobState::Failed
    );
    assert!(timer.poll(&mut Refusing, now, wall).is_err());
    let mut timer = LocalTimer::activate(&db, &activation, now, wall).unwrap();
    assert!(
        timer
            .poll(&mut Refusing, now + Duration::from_secs(10), wall)
            .is_err()
    );
    assert_eq!(
        db.job(&scopes, timer.id()).unwrap().unwrap().state,
        JobState::Running
    );
    drop(timer);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n42_timer_requires_scope_and_current_schedule_lease() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let policy = SyncPolicy {
        mode: SyncMode::Watch,
        timer_period_ms: NonZeroU64::new(86_400_000),
        overlap_ms: 0,
        clock_skew_ms: 0,
        revision_fields: vec![],
    };
    let mut activation = Activation {
        policy: &policy,
        scope: &scope,
        principal: "owner",
        mode: Mode::Full,
        lease_term: Duration::from_secs(10),
    };
    let now = Instant::now();
    let wall = UNIX_EPOCH + Duration::from_secs(2_000_000);
    assert!(LocalTimer::activate(&db, &activation, now, wall).is_err());
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    let manual = SyncPolicy {
        mode: SyncMode::Manual,
        ..policy.clone()
    };
    assert!(
        LocalTimer::activate(
            &db,
            &Activation {
                policy: &manual,
                ..activation
            },
            now,
            wall
        )
        .is_err()
    );
    activation.lease_term = Duration::ZERO;
    assert!(LocalTimer::activate(&db, &activation, now, wall).is_err());
    activation.lease_term = Duration::from_secs(10);
    let mut timer = LocalTimer::activate(&db, &activation, now, wall).unwrap();
    assert!(LocalTimer::activate(&db, &activation, now, wall).is_err());
    assert!(timer.poll(&mut Refusing, now, wall).unwrap().is_none());
    let request = timer.request(TriggerKind::Manual);
    let mut trigger = Trigger::default();
    db.revoke("owner", &scope, Right::Read, "test").unwrap();
    assert!(timer.invoke(&mut trigger, &request, now, wall).is_err());
    assert!(trigger.0.is_empty());
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    db.take_job(
        timer.id(),
        "successor",
        wall + Duration::from_secs(11),
        Duration::from_secs(10),
    )
    .unwrap();
    assert!(timer.invoke(&mut trigger, &request, now, wall).is_err());
    assert!(trigger.0.is_empty());
    drop(timer);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

/// Records only admitted trigger invocations, never effects or authority.
#[derive(Default)]
pub(super) struct Trigger(pub(super) Vec<SyncRequest>);
impl ScheduleTrigger for Trigger {
    fn invoke(&mut self, request: &SyncRequest, _: &str) -> Result<Handle, Refusal> {
        self.0.push(request.clone());
        Ok(Handle::new())
    }
}
/// Independently authored strict scheduling policy.
pub(super) fn policy(mode: SyncMode, cadence: Option<u64>) -> SyncPolicy {
    SyncPolicy {
        mode,
        timer_period_ms: cadence.and_then(NonZeroU64::new),
        overlap_ms: 0,
        clock_skew_ms: 0,
        revision_fields: vec![],
    }
}
#[test]
fn n42_fake_clock_manual_one_off_watch_and_replay() {
    let now = Instant::now();
    for mode in [SyncMode::Manual, SyncMode::OneOff, SyncMode::Watch] {
        let mut schedule = Schedule::new(
            &policy(mode, Some(86_400_000)),
            Handle::new(),
            "owner",
            Mode::Incremental,
            now,
        )
        .unwrap();
        let mut trigger = Trigger::default();
        let request = schedule.request(TriggerKind::Timer);
        let due = now + Duration::from_hours(24);
        if mode == SyncMode::Manual {
            assert!(
                schedule
                    .invoke(&mut trigger, &request, "owner", due)
                    .is_err()
            );
            let manual = schedule.request(TriggerKind::Manual);
            schedule
                .invoke(&mut trigger, &manual, "owner", now)
                .unwrap();
        } else {
            if mode == SyncMode::Watch {
                assert!(
                    schedule
                        .invoke(
                            &mut trigger,
                            &request,
                            "owner",
                            due.checked_sub(Duration::from_nanos(1)).unwrap()
                        )
                        .is_err()
                );
            }
            assert!(
                schedule
                    .invoke(&mut trigger, &request, "foreign", due)
                    .is_err()
            );
            schedule
                .invoke(&mut trigger, &request, "owner", due)
                .unwrap();
            assert!(
                schedule
                    .invoke(&mut trigger, &request, "owner", due)
                    .is_err()
            );
        }
        assert_eq!(trigger.0.len(), 1);
        let next = schedule.request(TriggerKind::Timer);
        if mode == SyncMode::OneOff || mode == SyncMode::Watch {
            assert!(schedule.invoke(&mut trigger, &next, "owner", due).is_err());
        }
        let stale = schedule.request(TriggerKind::Manual);
        schedule.stop();
        assert!(
            schedule
                .invoke(
                    &mut trigger,
                    &stale,
                    "owner",
                    due + Duration::from_hours(24)
                )
                .is_err()
        );
        assert_eq!(trigger.0.len(), 1);
    }
}

#[test]
fn n42_requires_explicit_approved_watch_cadence() {
    let now = Instant::now();
    assert!(
        Schedule::new(
            &policy(SyncMode::Manual, None),
            Handle::new(),
            "",
            Mode::Full,
            now
        )
        .is_err()
    );
    for cadence in [None, Some(1), Some(86_399_999)] {
        assert!(
            Schedule::new(
                &policy(SyncMode::Watch, cadence),
                Handle::new(),
                "owner",
                Mode::Full,
                now
            )
            .is_err()
        );
    }
    assert!(
        Schedule::new(
            &policy(SyncMode::Watch, Some(86_400_000)),
            Handle::new(),
            "owner",
            Mode::Full,
            now
        )
        .is_ok()
    );
}

#[test]
fn n42_wall_expiry_cannot_be_renewed_or_undone_by_clock_rollback() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    let policy = policy(SyncMode::Watch, Some(86_400_000));
    let now = Instant::now();
    let wall = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let mut timer = LocalTimer::activate(
        &db,
        &Activation {
            policy: &policy,
            scope: &scope,
            principal: "owner",
            mode: Mode::Full,
            lease_term: Duration::from_secs(10),
        },
        now,
        wall,
    )
    .unwrap();
    let mut trigger = Trigger::default();
    let request = timer.request(TriggerKind::Manual);
    assert!(
        timer
            .invoke(&mut trigger, &request, now, wall + Duration::from_secs(10))
            .is_err()
    );
    assert!(trigger.0.is_empty());
    assert!(timer.invoke(&mut trigger, &request, now, wall).is_err());
    assert!(trigger.0.is_empty());
    drop(timer);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n42_request_contract_refuses_each_substitution_without_consuming() {
    let now = Instant::now();
    let mut schedule = Schedule::new(
        &policy(SyncMode::Manual, None),
        Handle::new(),
        "owner",
        Mode::Incremental,
        now,
    )
    .unwrap();
    let original = schedule.request(TriggerKind::Manual);
    let mut trigger = Trigger::default();
    for substituted in [
        SyncRequest {
            activation: Handle::new(),
            ..original.clone()
        },
        SyncRequest {
            sequence: 1,
            ..original.clone()
        },
        SyncRequest {
            mode: Mode::Full,
            ..original.clone()
        },
    ] {
        assert!(
            schedule
                .invoke(&mut trigger, &substituted, "owner", now)
                .is_err()
        );
        assert_eq!(schedule.request(TriggerKind::Manual), original);
    }
    assert!(trigger.0.is_empty());
    schedule
        .invoke(&mut trigger, &original, "owner", now)
        .unwrap();
    assert_eq!(trigger.0.len(), 1);
}

#[test]
fn n42_stop_is_durable_idempotent_owner_only_and_terminal() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    db.grant("foreign", &scope, Right::Read, "test").unwrap();
    let scopes = db.visible("owner").unwrap();
    let now = Instant::now();
    let wall = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let policy = policy(SyncMode::Watch, Some(86_400_000));
    let activation = Activation {
        policy: &policy,
        scope: &scope,
        principal: "owner",
        mode: Mode::Full,
        lease_term: Duration::from_secs(10),
    };
    let mut timer = LocalTimer::activate(&db, &activation, now, wall).unwrap();
    assert_eq!(
        db.job(&scopes, timer.id()).unwrap().unwrap().kind,
        "acquisition.schedule"
    );
    let request = timer.request(TriggerKind::Manual);
    assert!(request_stop(&db, &scopes, timer.id(), "foreign").is_err());
    assert!(!stop_requested(&db, &scopes, timer.id()).unwrap());
    request_stop(&db, &scopes, timer.id(), "owner").unwrap();
    let events = db
        .events(
            &scopes,
            &Filter {
                stream: &stream(timer.id()),
                after: 0,
                r#type: None,
            },
        )
        .unwrap();
    request_stop(&db, &scopes, timer.id(), "owner").unwrap();
    assert_eq!(
        db.events(
            &scopes,
            &Filter {
                stream: &stream(timer.id()),
                after: 0,
                r#type: None
            }
        )
        .unwrap(),
        events
    );
    let mut trigger = Trigger::default();
    assert!(timer.invoke(&mut trigger, &request, now, wall).is_err());
    assert_eq!(
        db.job(&scopes, timer.id()).unwrap().unwrap().state,
        JobState::Cancelled
    );
    assert!(timer.invoke(&mut trigger, &request, now, wall).is_err());
    assert!(trigger.0.is_empty());
    assert!(request_stop(&db, &scopes, timer.id(), "owner").is_err());
    let fresh = LocalTimer::activate(&db, &activation, now, wall).unwrap();
    assert_ne!(fresh.id(), timer.id());
    drop(fresh);
    drop(timer);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n42_stop_refuses_wrong_kind_missing_owner_and_scope_then_cancels_queued() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    let scopes = db.visible("owner").unwrap();
    let wall = UNIX_EPOCH + Duration::from_secs(2_000_000);
    assert!(request_stop(&db, &scopes, Ulid::generate(), "owner").is_err());
    for (kind, inputs) in [
        ("foreign.job", json!({"owner":"owner"})),
        ("acquisition.schedule", json!({})),
        ("acquisition.schedule", json!({"owner":"owner"})),
    ] {
        let job = db
            .submit_job(
                &NewJob {
                    kind,
                    inputs: &inputs,
                    scope: &scope,
                    resource: None,
                },
                wall,
            )
            .unwrap();
        if kind != "acquisition.schedule" || inputs.get("owner").is_none() {
            assert!(request_stop(&db, &scopes, job.id, "owner").is_err());
            assert!(!stop_requested(&db, &scopes, job.id).unwrap());
            continue;
        }
        db.revoke("owner", &scope, Right::Read, "test").unwrap();
        assert!(request_stop(&db, &db.visible("owner").unwrap(), job.id, "owner").is_err());
        db.grant("owner", &scope, Right::Read, "test").unwrap();
        request_stop(&db, &scopes, job.id, "owner").unwrap();
        assert_eq!(
            db.job(&scopes, job.id).unwrap().unwrap().state,
            JobState::Cancelled
        );
    }
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n42_stop_acknowledges_at_expiry() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    let policy = policy(SyncMode::Watch, Some(86_400_000));
    let activation = Activation {
        policy: &policy,
        scope: &scope,
        principal: "owner",
        mode: Mode::Full,
        lease_term: Duration::from_secs(10),
    };
    let now = Instant::now();
    let wall = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let scopes = db.visible("owner").unwrap();
    for elapsed in [Duration::from_secs(10), Duration::from_secs(11)] {
        let mut timer = LocalTimer::activate(&db, &activation, now, wall).unwrap();
        request_stop(&db, &scopes, timer.id(), "owner").unwrap();
        let mut trigger = Trigger::default();
        assert!(
            timer
                .poll(&mut trigger, now + elapsed, wall + elapsed)
                .is_err()
        );
        assert!(trigger.0.is_empty());
        assert!(stop_requested(&db, &scopes, timer.id()).unwrap());
        assert_eq!(
            db.job(&scopes, timer.id()).unwrap().unwrap().state,
            JobState::Cancelled
        );
    }
    drop(db);
    fs::remove_dir_all(root).unwrap();
}
