//! The lease of a job run in the foreground, which only `Holder::run` holds:
//! renewed at each heartbeat and each step while its work runs, never again
//! once another process took it over, and released with the outcome its
//! work returns.

use super::support::{Scratch, everything, leased, lost, recorded};
use crate::cli::lease::{Holder, TIMING, Timing, ticks};
use maestro_kernel::job::{self, JobState};
use serde_json::json;
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime},
};

/// Leases of an hour, which a renewal makes outlast one of a minute, and a
/// heartbeat thread that beats every millisecond.
const BEATING: Timing = Timing {
    term: Duration::from_secs(3600),
    beat: Duration::from_millis(1),
};

/// Leases of an hour, and a heartbeat thread that never beats while a test
/// runs: the test drives the ticks itself.
const QUIET: Timing = Timing {
    term: Duration::from_secs(3600),
    beat: Duration::from_secs(3600),
};

/// How long a test waits for a heartbeat before it fails: shorter than the
/// 20 s a mutation test is given at least, so that heartbeats that never come
/// fail the test instead of timing out.
const PATIENCE: Duration = Duration::from_secs(10);

#[test]
fn the_command_line_leases_last_a_minute_renewed_every_twenty_seconds() {
    assert_eq!(
        (TIMING.term, TIMING.beat),
        (Duration::from_secs(60), Duration::from_secs(20))
    );
}

#[test]
fn heartbeats_renew_the_lease_while_the_work_runs() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let taken = leased(&database, "cli", SystemTime::now(), Duration::from_secs(60));
    let deadline = Instant::now() + PATIENCE;
    let ended = Holder::run(&database, taken.clone(), BEATING, |_| {
        let renewed = loop {
            if recorded(&database, &scopes, &taken).expires > taken.expires {
                break true;
            }
            if Instant::now() > deadline {
                break false;
            }
        };
        (JobState::Succeeded, json!({ "renewed": renewed }))
    })
    .unwrap();
    assert_eq!(
        ended.outcome,
        Some(json!({ "renewed": true })),
        "a heartbeat renewed the lease within {PATIENCE:?}"
    );
}

#[test]
fn a_holder_renews_its_lease_at_each_tick_until_the_ticks_end() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let taken = leased(&database, "cli", SystemTime::now(), Duration::from_secs(60));
    Holder::run(&database, taken.clone(), QUIET, |holder| {
        let (tick, ticks) = mpsc::sync_channel(0);
        thread::scope(|scope| {
            let beating = scope.spawn(move || holder.beat_at(ticks.into_iter()));
            tick.send(()).unwrap();
            // Taken only once the renewal of the first tick is done.
            tick.send(()).unwrap();
            let renewed = recorded(&database, &scopes, &taken);
            assert!(
                renewed.expires > taken.expires,
                "{renewed:?} renews {taken:?}"
            );
            drop(tick);
            beating.join().unwrap();
        });
        (JobState::Succeeded, json!({}))
    })
    .unwrap();
}

#[test]
fn a_holder_stops_beating_once_another_took_its_lease_over() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let taken = lost(&database, "cli");
    let ended = Holder::run(&database, taken.clone(), QUIET, |holder| {
        let (tick, ticks) = mpsc::sync_channel(0);
        thread::scope(|scope| {
            scope.spawn(move || holder.beat_at(ticks.into_iter()));
            tick.send(()).unwrap();
            assert!(
                tick.send(()).is_err(),
                "the holder stopped at the renewal that found its lease lost"
            );
        });
        (JobState::Succeeded, json!({}))
    });
    assert!(matches!(ended, Err(job::Error::Lost { .. })), "{ended:?}");
    assert_eq!(recorded(&database, &scopes, &taken).holder, "successor");
}

#[test]
fn a_step_is_journaled_under_the_lease_and_the_job_ends_as_its_work_says() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let taken = leased(&database, "cli", SystemTime::now(), Duration::from_secs(60));
    let step = json!({ "imported": 1 });
    let outcome = json!({ "done": true });
    let ended = Holder::run(&database, taken.clone(), QUIET, |holder| {
        holder.step(&step).unwrap();
        let progressed = database.last_progress(&scopes, taken.job).unwrap().unwrap();
        assert_eq!(progressed.data, step);
        assert!(recorded(&database, &scopes, &taken).expires > taken.expires);
        (JobState::Failed, outcome.clone())
    })
    .unwrap();
    assert_eq!(
        (ended.id, ended.state, ended.outcome),
        (taken.job, JobState::Failed, Some(outcome))
    );
}

#[test]
fn a_holder_that_lost_its_lease_writes_nothing() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let taken = lost(&database, "cli");
    let ended = Holder::run(&database, taken.clone(), QUIET, |holder| {
        assert!(holder.step(&json!({ "imported": 1 })).is_err());
        (JobState::Succeeded, json!({}))
    });
    assert!(matches!(ended, Err(job::Error::Lost { .. })), "{ended:?}");
    assert_eq!(database.last_progress(&scopes, taken.job).unwrap(), None);
}

#[test]
fn ticks_come_every_beat_until_their_sender_goes() {
    let (stop, stopped) = mpsc::channel::<()>();
    let beat = Duration::from_millis(1);
    let mut beating = ticks(stopped, beat);
    assert_eq!(
        beating.by_ref().take(3).count(),
        3,
        "a tick after each beat"
    );
    drop(stop);
    assert_eq!(beating.next(), None, "no tick once the sender went");
}
