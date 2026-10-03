//! Synthetic scheduling and public stop boundaries; no live activation grant.
use super::support::Home;
use maestro_acquisition::{
    Refusal,
    lifecycle::{
        full::Mode,
        schedule::{
            Activation, JOB_KIND, LocalTimer, ScheduleTrigger, SyncRequest, TriggerKind,
            stop_requested,
        },
    },
    policy::source::{SyncMode, SyncPolicy},
};
use maestro_kernel::{
    acquisition::{Frontier, Handle, LeaseRequest, NewItem},
    artifact::Digest,
    job::{JobState, NewJob, stream},
    journal::Filter,
    scope::{LOCAL, Scope},
    store::Database,
};
use serde_json::json;
use std::{
    env, fs,
    num::NonZeroU64,
    path::PathBuf,
    process::Command,
    sync::mpsc,
    time::{Duration, Instant, SystemTime},
};

/// Records dispatches without authority, network or an S4 runtime.
#[derive(Default)]
struct Trigger(Vec<SyncRequest>);
impl ScheduleTrigger for Trigger {
    fn invoke(&mut self, request: &SyncRequest, _: &str) -> Result<Handle, Refusal> {
        self.0.push(request.clone());
        Ok(Handle::new())
    }
}

/// Strict source sync declaration with a caller-selected explicit cadence.
fn policy(mode: SyncMode, cadence: Option<u64>) -> SyncPolicy {
    SyncPolicy {
        mode,
        timer_period_ms: cadence.and_then(NonZeroU64::new),
        overlap_ms: 0,
        clock_skew_ms: 0,
        revision_fields: vec![],
    }
}

#[test]
fn n42_live_timer_refuses_without_activation_effect() {
    let home = Home::new();
    let result = home.run(&["--json", "knowledge", "acquire", "timer"]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(
        result.stdout.contains("active-source activation effect"),
        "{result:?}"
    );
    assert!(!home.data().join("kernel.sqlite3").exists());
}

#[test]
fn n42_stop_refuses_missing_foreign_and_wrong_kind_jobs() {
    let home = Home::new();
    let db = home.database();
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    db.refresh_config(&home.config()).unwrap();
    for (kind, owner) in [
        ("acquisition.schedule", "foreign"),
        ("knowledge.import", LOCAL),
    ] {
        let job = db
            .submit_job(
                &NewJob {
                    kind,
                    inputs: &json!({"owner":owner}),
                    scope: &scope,
                    resource: None,
                },
                SystemTime::now(),
            )
            .unwrap();
        let id = job.id.to_string();
        let result = home.run(&[
            "--json",
            "knowledge",
            "acquire",
            "stop",
            "--schedule",
            &id,
            "--deadline-ms",
            "100",
        ]);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(
            result.stdout.contains("no owned live acquisition schedule"),
            "{result:?}"
        );
        assert_eq!(
            db.job(&db.visible(LOCAL).unwrap(), job.id)
                .unwrap()
                .unwrap()
                .state,
            JobState::Queued
        );
    }
    let id = Handle::new().to_string();
    let result = home.run(&["--json", "knowledge", "acquire", "stop", "--schedule", &id]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(
        result.stdout.contains("no owned live acquisition schedule"),
        "{result:?}"
    );
}

#[test]
fn n42_stop_deadline_is_a_bounded_pi_input() {
    let home = Home::new();
    let id = Handle::new().to_string();
    for deadline in ["0", "2147483648", "-1", "nan", "1.5"] {
        let result = home.run(&[
            "knowledge",
            "acquire",
            "stop",
            "--schedule",
            &id,
            "--deadline-ms",
            deadline,
        ]);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(result.stderr.contains("error:"), "{result:?}");
        assert!(!home.data().join("kernel.sqlite3").exists());
    }
}

/// A child test process runs the actual local adapter; it has no S3/S4 host.
#[test]
fn n42_timer_child_observes_durable_stop() {
    let Some(root) = env::var_os("MAESTRO_N42_TIMER_HOME") else {
        return;
    };
    let root = PathBuf::from(root);
    let db = Database::open_in(&root.join("data/maestro")).unwrap();
    db.refresh_config(&root.join("config/maestro")).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let declaration = policy(SyncMode::Watch, Some(86_400_000));
    let mut timer = LocalTimer::activate(
        &db,
        &Activation {
            policy: &declaration,
            scope: &scope,
            principal: LOCAL,
            mode: Mode::Full,
            lease_term: Duration::from_secs(15),
        },
        Instant::now(),
        SystemTime::now(),
    )
    .unwrap();
    let stale = timer.request(TriggerKind::Timer);
    fs::write(root.join("timer-ready"), timer.id().to_string()).unwrap();
    let mut trigger = Trigger::default();
    let (_wake, timer_signal) = mpsc::channel::<()>();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match timer.poll(&mut trigger, Instant::now(), SystemTime::now()) {
            Ok(None) => assert!(timer_signal.recv_timeout(Duration::from_millis(1)).is_err()),
            Ok(Some(_)) => panic!("unexpected fetch before approved cadence"),
            Err(_) => break,
        }
        assert!(Instant::now() < deadline, "stop was not observed");
    }
    assert_eq!(
        db.job(&db.visible(LOCAL).unwrap(), timer.id())
            .unwrap()
            .unwrap()
            .state,
        JobState::Cancelled
    );
    assert!(
        timer
            .invoke(
                &mut trigger,
                &stale,
                Instant::now() + Duration::from_hours(24),
                SystemTime::now()
            )
            .is_err()
    );
    assert!(trigger.0.is_empty());
}

#[test]
fn n42_cli_stop_disables_owned_real_timer_and_retains_pending() {
    let home = Home::new();
    let db = home.database();
    let scopes = db.refresh_config(&home.config()).unwrap();
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    let now = SystemTime::now();
    let writer = db
        .lease_source(
            "n42-pending",
            &scope,
            LeaseRequest {
                holder: LOCAL,
                now,
                term: Duration::from_secs(15),
            },
        )
        .unwrap();
    db.enqueue(
        &writer,
        &NewItem {
            fetch_identity: "https://synthetic.example/pending".into(),
            authorization_context: Digest::of(b"authorized"),
            representation_profile: Digest::of(b"profile"),
        },
        now,
    )
    .unwrap();
    db.release_source(&writer, now).unwrap();
    let before = db.page(&scopes, "n42-pending", None, 1000).unwrap();
    let mut child = Command::new(env::current_exe().unwrap())
        .args([
            "--exact",
            concat!(
                "n42_add_local_recurring_triggers_and_stop_controls::",
                "n42_timer_child_observes_durable_stop"
            ),
            "--nocapture",
        ])
        .env("MAESTRO_N42_TIMER_HOME", home.root())
        .spawn()
        .unwrap();
    let (_wake, timer_signal) = mpsc::channel::<()>();
    let deadline = Instant::now() + Duration::from_secs(15);
    let ready = home.root().join("timer-ready");
    while !ready.exists() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("timer did not start");
        }
        assert!(timer_signal.recv_timeout(Duration::from_millis(1)).is_err());
    }
    let id = fs::read_to_string(ready).unwrap();
    let result = home.run(&[
        "--json",
        "knowledge",
        "acquire",
        "stop",
        "--schedule",
        &id,
        "--deadline-ms",
        "15000",
    ]);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("owned timer did not exit");
        }
        assert!(timer_signal.recv_timeout(Duration::from_millis(1)).is_err());
    }
    assert!(child.wait().unwrap().success());
    assert_eq!(result.code, Some(0), "{result:?}");
    assert_eq!(db.page(&scopes, "n42-pending", None, 1000).unwrap(), before);
    assert_eq!(
        db.job(&scopes, id.parse().unwrap()).unwrap().unwrap().state,
        JobState::Cancelled
    );
    let repeat = home.run(&["--json", "knowledge", "acquire", "stop", "--schedule", &id]);
    assert_eq!(
        repeat.code,
        Some(2),
        "stopped activation was resurrected: {repeat:?}"
    );
}

#[test]
fn n42_stop_timeout_is_not_cancellation_and_stays_durable() {
    let home = Home::new();
    let db = home.database();
    let scopes = db.refresh_config(&home.config()).unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let job = db
        .submit_job(
            &NewJob {
                kind: JOB_KIND,
                inputs: &json!({"owner":LOCAL}),
                scope: &scope,
                resource: None,
            },
            SystemTime::now(),
        )
        .unwrap();
    db.take_job(job.id, LOCAL, SystemTime::now(), Duration::from_secs(15))
        .unwrap();
    let result = home.run(&[
        "--json",
        "knowledge",
        "acquire",
        "stop",
        "--schedule",
        &job.id.to_string(),
        "--deadline-ms",
        "100",
    ]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(
        result.stdout.contains("cancellation is not acknowledged"),
        "{result:?}"
    );
    assert_eq!(
        db.job(&scopes, job.id).unwrap().unwrap().state,
        JobState::Running
    );
    assert!(stop_requested(&db, &scopes, job.id).unwrap());
    let retry = home.run(&[
        "--json",
        "knowledge",
        "acquire",
        "stop",
        "--schedule",
        &job.id.to_string(),
        "--deadline-ms",
        "100",
    ]);
    assert_eq!(retry.code, Some(2));
    assert!(stop_requested(&db, &scopes, job.id).unwrap());
    let count = db
        .events(
            &scopes,
            &Filter {
                stream: &stream(job.id),
                after: 0,
                r#type: None,
            },
        )
        .unwrap()
        .len();
    assert_eq!(count, 3, "created, taken, one idempotent stop request");
}
