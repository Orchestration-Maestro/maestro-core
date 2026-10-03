//! Distinguishing cadence, outcome, renewal and fractional expiry assertions.
use super::n42_schedule::{Trigger, policy};
use maestro_acquisition::{
    Refusal,
    lifecycle::{
        full::Mode,
        schedule::{Activation, LocalTimer, Schedule, TriggerKind},
    },
    policy::source::SyncMode,
};
use maestro_kernel::{acquisition::Handle, job::JobState, scope::Right, store::Database};
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    time::{Duration, Instant, UNIX_EPOCH},
};

#[test]
fn n42_delayed_watch_anchors_next_tick_to_dispatch_time() {
    let now = Instant::now();
    let mut schedule = Schedule::new(
        &policy(SyncMode::Watch, Some(86_400_000)),
        Handle::new(),
        "owner",
        Mode::Full,
        now,
    )
    .unwrap();
    let mut trigger = Trigger::default();
    let delayed = now + Duration::from_hours(49);
    schedule
        .invoke(
            &mut trigger,
            &schedule.request(TriggerKind::Timer),
            "owner",
            delayed,
        )
        .unwrap();
    let next = schedule.request(TriggerKind::Timer);
    assert_eq!(
        schedule.invoke(
            &mut trigger,
            &next,
            "owner",
            delayed + Duration::from_hours(23)
        ),
        Err(Refusal::Access)
    );
    assert_eq!(trigger.0.len(), 1);
    schedule
        .invoke(
            &mut trigger,
            &next,
            "owner",
            delayed + Duration::from_hours(24),
        )
        .unwrap();
    assert_eq!(trigger.0.len(), 2);
}

#[test]
fn n42_one_off_success_retains_state_and_run_outcome() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    let policy = policy(SyncMode::OneOff, None);
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
    let run = timer
        .poll(&mut Trigger::default(), now, wall)
        .unwrap()
        .unwrap();
    let job = db
        .job(&db.visible("owner").unwrap(), timer.id())
        .unwrap()
        .unwrap();
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(job.outcome.unwrap(), serde_json::json!({"run":run}));
    drop(timer);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n42_heartbeat_extends_monotonic_ownership() {
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
    assert!(
        timer
            .poll(
                &mut trigger,
                now + Duration::from_secs(5),
                wall + Duration::from_secs(5)
            )
            .unwrap()
            .is_none()
    );
    timer
        .invoke(
            &mut trigger,
            &timer.request(TriggerKind::Manual),
            now + Duration::from_secs(11),
            wall + Duration::from_secs(11),
        )
        .unwrap();
    assert_eq!(trigger.0.len(), 1);
    drop(timer);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n42_fractional_wall_expiry_refuses_without_dispatch() {
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
            lease_term: Duration::from_millis(1500),
        },
        now,
        wall,
    )
    .unwrap();
    let mut trigger = Trigger::default();
    assert_eq!(
        timer.invoke(
            &mut trigger,
            &timer.request(TriggerKind::Manual),
            now,
            wall + Duration::from_millis(1600)
        ),
        Err(Refusal::Access)
    );
    assert!(trigger.0.is_empty());
    drop(timer);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

/// Largest whole-second displacement this host's Instant can represent.
fn last_instant(now: Instant) -> Instant {
    let (mut low, mut high) = (0, u64::MAX);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if now.checked_add(Duration::from_secs(middle)).is_some() {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    now.checked_add(Duration::from_secs(low)).unwrap()
}

#[test]
fn n42_clock_addition_overflows_refuse_without_dispatch() {
    let now = Instant::now();
    let end = last_instant(now);
    let watch = policy(SyncMode::Watch, Some(86_400_000));
    assert!(matches!(
        Schedule::new(&watch, Handle::new(), "owner", Mode::Full, end),
        Err(Refusal::Invalid)
    ));
    let mut schedule = Schedule::new(&watch, Handle::new(), "owner", Mode::Full, now).unwrap();
    let mut trigger = Trigger::default();
    assert_eq!(
        schedule.invoke(
            &mut trigger,
            &schedule.request(TriggerKind::Manual),
            "owner",
            end
        ),
        Err(Refusal::Invalid)
    );
    assert!(trigger.0.is_empty());
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    let one_off = policy(SyncMode::OneOff, None);
    let wall = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let activation = Activation {
        policy: &one_off,
        scope: &scope,
        principal: "owner",
        mode: Mode::Full,
        lease_term: Duration::from_secs(10),
    };
    assert!(matches!(
        LocalTimer::activate(&db, &activation, end, wall),
        Err(Refusal::Invalid)
    ));
    let start = end.checked_sub(activation.lease_term).unwrap();
    let mut timer = LocalTimer::activate(&db, &activation, start, wall).unwrap();
    assert_eq!(
        timer.poll(
            &mut trigger,
            end.checked_sub(Duration::from_secs(1)).unwrap(),
            wall
        ),
        Err(Refusal::Invalid)
    );
    assert!(trigger.0.is_empty());
    drop(timer);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}
