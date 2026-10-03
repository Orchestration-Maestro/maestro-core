//! Dead-owner recovery and lease timing refusal contracts.
use maestro_acquisition::{
    lifecycle::{
        full::Mode,
        schedule::{Activation, JOB_KIND, LocalTimer, TriggerKind, request_stop, stop_requested},
    },
    policy::source::{SyncMode, SyncPolicy},
};
use maestro_kernel::{
    job::{JobState, NewJob},
    scope::{Right, Scope},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use serde_json::json;
use std::{
    fs,
    num::NonZeroU64,
    time::{Duration, Instant, UNIX_EPOCH},
};

#[test]
fn n42_expired_owner_stop_allows_new_activation() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    db.grant("owner", &scope, Right::Read, "test").unwrap();
    let scopes = db.visible("owner").unwrap();
    let policy = SyncPolicy {
        mode: SyncMode::Watch,
        timer_period_ms: NonZeroU64::new(86_400_000),
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
    let timer = LocalTimer::activate(&db, &activation, now, wall).unwrap();
    let id = timer.id();
    drop(timer);
    request_stop(&db, &scopes, id, "owner").unwrap();
    assert!(LocalTimer::activate(&db, &activation, now, wall).is_err());
    assert_eq!(
        db.job(&scopes, id).unwrap().unwrap().state,
        JobState::Running
    );
    let later = Duration::from_secs(11);
    let fresh = LocalTimer::activate(&db, &activation, now + later, wall + later).unwrap();
    assert_ne!(fresh.id(), id);
    assert_eq!(fresh.request(TriggerKind::Timer).sequence, 0);
    assert_eq!(
        db.job(&scopes, id).unwrap().unwrap().state,
        JobState::Cancelled
    );
    assert!(stop_requested(&db, &scopes, id).unwrap());
    drop(fresh);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n42_recovery_never_takes_foreign_wrong_kind_or_unstopped_holder() {
    for (kind, owner, holder, stopped) in [
        (JOB_KIND, "foreign", "foreign", true),
        ("other.kind", "owner", "owner", true),
        (JOB_KIND, "owner", "successor", true),
        (JOB_KIND, "owner", "owner", false),
    ] {
        let root = scratch_directory().unwrap();
        let db = Database::open_in(&root).unwrap();
        let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
        db.grant("owner", &scope, Right::Read, "test").unwrap();
        db.grant("foreign", &scope, Right::Read, "test").unwrap();
        let scopes = db.visible("owner").unwrap();
        let now = Instant::now();
        let wall = UNIX_EPOCH + Duration::from_secs(2_000_000);
        let resource = format!("acquisition/schedule/{}", scope.as_str());
        let job = db
            .submit_job(
                &NewJob {
                    kind,
                    inputs: &json!({"owner":owner}),
                    scope: &scope,
                    resource: Some(&resource),
                },
                wall,
            )
            .unwrap();
        let lease = db
            .take_job(job.id, holder, wall, Duration::from_secs(10))
            .unwrap();
        if stopped {
            // Wrong-kind jobs cannot request stop through the schedule API.
            if kind == JOB_KIND {
                request_stop(&db, &scopes, job.id, owner).unwrap();
            }
        }
        let before = db.job(&scopes, job.id).unwrap().unwrap();
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
        assert!(
            LocalTimer::activate(
                &db,
                &activation,
                now + Duration::from_secs(11),
                wall + Duration::from_secs(11)
            )
            .is_err()
        );
        assert_eq!(db.job(&scopes, job.id).unwrap().unwrap(), before);
        db.complete_job(&lease, JobState::Cancelled, &json!({}))
            .unwrap();
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn n42_failed_take_does_not_reserve_schedule() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
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
    let scopes = db.visible("owner").unwrap();
    assert!(
        LocalTimer::activate(
            &db,
            &activation,
            now,
            UNIX_EPOCH + Duration::from_secs(253_402_300_799)
        )
        .is_err()
    );
    assert!(
        db.unfinished_jobs(&scopes, &format!("acquisition/schedule/{}", scope.as_str()))
            .unwrap()
            .is_empty()
    );
    assert!(
        db.jobs_for_resource(
            &scopes,
            JOB_KIND,
            &format!("acquisition/schedule/{}", scope.as_str())
        )
        .unwrap()
        .is_empty(),
        "invalid timing must refuse before submission"
    );
    assert!(
        LocalTimer::activate(&db, &activation, now, UNIX_EPOCH - Duration::from_secs(1)).is_err()
    );
    assert!(
        LocalTimer::activate(
            &db,
            &activation,
            now,
            UNIX_EPOCH + Duration::from_secs(2_000_000)
        )
        .is_ok()
    );
    drop(db);
    fs::remove_dir_all(root).unwrap();
}
