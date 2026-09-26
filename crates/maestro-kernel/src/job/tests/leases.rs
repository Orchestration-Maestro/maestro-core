//! Leases: one holder at a time, taken over once expired, renewed by
//! heartbeats, and a stale holder refused once another took the lease over.

use super::support::{FIRST, SECOND, Scratch, TERM, at, collection, journaled, publish, running};
use crate::{
    job::{Error, JobState, Lease},
    scope::ScopeSet,
};
use serde_json::json;
use std::time::{Duration, UNIX_EPOCH};
use ulid::Ulid;

#[test]
fn a_second_lease_on_the_same_key_is_refused_naming_the_holder_and_the_expiry() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let inputs = collection("demo");
    let job = database.submit_job(&publish(&inputs), at(0)).unwrap();
    let lease = database.take_job(job.id, FIRST, at(0), TERM).unwrap();
    assert_eq!(
        lease,
        Lease {
            job: job.id,
            holder: FIRST.to_owned(),
            number: 1,
            heartbeat: "2026-09-26T12:00:00.000Z".to_owned(),
            expires: "2026-09-26T12:00:30.000Z".to_owned(),
        }
    );
    let retried = database.submit_job(&publish(&inputs), at(29)).unwrap();
    assert_eq!(retried.id, job.id, "the retried command finds the job");
    let last_moment = at(29) + Duration::from_millis(999);
    for taker in [SECOND, FIRST] {
        let refused = database
            .take_job(retried.id, taker, last_moment, TERM)
            .unwrap_err();
        assert!(
            matches!(
                &refused,
                Error::Held { job: held, holder, expires }
                    if *held == job.id
                        && holder == FIRST
                        && expires == "2026-09-26T12:00:30.000Z"
            ),
            "{refused:?}"
        );
    }
    let unchanged = database
        .job(&ScopeSet::default_workspace(), job.id)
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.state, JobState::Running);
    assert_eq!(unchanged.lease, Some(lease));
}

#[test]
fn an_expired_lease_is_taken_over_and_its_job_keeps_running_under_the_new_holder() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, _) = running(&database, "demo");
    let taken = database.take_job(job.id, SECOND, at(30), TERM).unwrap();
    assert_eq!(
        taken,
        Lease {
            job: job.id,
            holder: SECOND.to_owned(),
            number: 2,
            heartbeat: "2026-09-26T12:00:30.000Z".to_owned(),
            expires: "2026-09-26T12:01:00.000Z".to_owned(),
        }
    );
    let job = database
        .job(&ScopeSet::default_workspace(), job.id)
        .unwrap()
        .unwrap();
    assert_eq!(job.state, JobState::Running);
    assert_eq!(job.lease, Some(taken));
}

#[test]
fn a_lease_expires_when_its_own_term_ends_whatever_term_a_taker_asks_for() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let job = database
        .submit_job(&publish(&collection("demo")), at(0))
        .unwrap();
    let minute = Duration::from_secs(60);
    database.take_job(job.id, FIRST, at(0), minute).unwrap();
    for now in [at(30), at(59) + Duration::from_millis(999)] {
        let refused = database.take_job(job.id, SECOND, now, TERM).unwrap_err();
        assert!(
            matches!(
                &refused,
                Error::Held { holder, expires, .. }
                    if holder == FIRST && expires == "2026-09-26T12:01:00.000Z"
            ),
            "{now:?}: {refused:?}"
        );
    }
    let taken = database.take_job(job.id, SECOND, at(60), TERM).unwrap();
    assert_eq!(
        (taken.number, taken.expires.as_str()),
        (2, "2026-09-26T12:01:30.000Z")
    );
}

#[test]
fn a_heartbeat_renews_the_lease_from_its_time_and_is_not_journaled() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, mut lease) = running(&database, "demo");
    let journal = journaled(&database, job.id);
    database.heartbeat(&mut lease, at(20), TERM).unwrap();
    assert_eq!(
        lease,
        Lease {
            job: job.id,
            holder: FIRST.to_owned(),
            number: 1,
            heartbeat: "2026-09-26T12:00:20.000Z".to_owned(),
            expires: "2026-09-26T12:00:50.000Z".to_owned(),
        }
    );
    assert_eq!(
        database
            .job(&ScopeSet::default_workspace(), job.id)
            .unwrap()
            .unwrap()
            .lease,
        Some(lease.clone())
    );
    let refused = database.take_job(job.id, SECOND, at(30), TERM).unwrap_err();
    assert!(
        matches!(&refused, Error::Held { expires, .. } if expires == "2026-09-26T12:00:50.000Z"),
        "{refused:?}"
    );
    assert_eq!(
        journaled(&database, job.id),
        journal,
        "no heartbeat is an event, nor a refused lease"
    );
}

#[test]
fn a_holder_keeps_an_expired_lease_until_another_takes_it_over() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, mut lease) = running(&database, "demo");
    database.heartbeat(&mut lease, at(45), TERM).unwrap();
    assert_eq!(lease.expires, "2026-09-26T12:01:15.000Z");
    assert_eq!(
        database
            .job(&ScopeSet::default_workspace(), job.id)
            .unwrap()
            .unwrap()
            .lease,
        Some(lease)
    );
}

#[test]
fn a_stale_holder_is_refused_once_its_lease_was_taken_over_and_writes_nothing() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, mut stale) = running(&database, "demo");
    let mut successor = database.take_job(job.id, SECOND, at(30), TERM).unwrap();
    let before = database
        .job(&ScopeSet::default_workspace(), job.id)
        .unwrap();
    let journal = journaled(&database, job.id);
    let step = json!({"step": 1});
    let refusals = [
        database.heartbeat(&mut stale, at(31), TERM).unwrap_err(),
        database
            .progress(&mut stale, at(31), TERM, &step)
            .unwrap_err(),
        database
            .complete_job(&stale, JobState::Succeeded, &step)
            .unwrap_err(),
    ];
    for refused in refusals {
        assert!(
            matches!(
                &refused,
                Error::Lost { job: lost, holder, number: 1 } if *lost == job.id && holder == FIRST
            ),
            "{refused:?}"
        );
    }
    assert_eq!(stale.heartbeat, "2026-09-26T12:00:00.000Z");
    assert_eq!(
        database
            .job(&ScopeSet::default_workspace(), job.id)
            .unwrap(),
        before
    );
    assert_eq!(journaled(&database, job.id), journal);
    database
        .progress(&mut successor, at(32), TERM, &step)
        .unwrap();
}

#[test]
fn a_lease_is_known_by_its_number_not_by_the_name_of_its_holder() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, mut stale) = running(&database, "demo");
    let mut restarted = database.take_job(job.id, FIRST, at(30), TERM).unwrap();
    assert_eq!((restarted.holder.as_str(), restarted.number), (FIRST, 2));
    let before = database
        .job(&ScopeSet::default_workspace(), job.id)
        .unwrap();
    let journal = journaled(&database, job.id);
    let step = json!({"step": 1});
    let refusals = [
        database.heartbeat(&mut stale, at(31), TERM).unwrap_err(),
        database
            .progress(&mut stale, at(31), TERM, &step)
            .unwrap_err(),
        database
            .complete_job(&stale, JobState::Succeeded, &step)
            .unwrap_err(),
    ];
    for refused in refusals {
        assert!(
            matches!(
                &refused,
                Error::Lost { job: lost, holder, number: 1 } if *lost == job.id && holder == FIRST
            ),
            "{refused:?}"
        );
    }
    assert_eq!(
        database
            .job(&ScopeSet::default_workspace(), job.id)
            .unwrap(),
        before
    );
    assert_eq!(journaled(&database, job.id), journal);
    database.heartbeat(&mut restarted, at(31), TERM).unwrap();
    database
        .progress(&mut restarted, at(32), TERM, &step)
        .unwrap();
}

#[test]
fn a_lease_on_a_job_the_kernel_does_not_record_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let unknown = Ulid::nil();
    let refused = database.take_job(unknown, FIRST, at(0), TERM).unwrap_err();
    assert!(
        matches!(refused, Error::UnknownJob(id) if id == unknown),
        "{refused:?}"
    );
    let mut forged = Lease {
        job: unknown,
        holder: FIRST.to_owned(),
        number: 1,
        heartbeat: "2026-09-26T12:00:00.000Z".to_owned(),
        expires: "2026-09-26T12:00:30.000Z".to_owned(),
    };
    let refused = database.heartbeat(&mut forged, at(1), TERM).unwrap_err();
    assert!(
        matches!(refused, Error::UnknownJob(id) if id == unknown),
        "{refused:?}"
    );
}

#[test]
fn a_time_the_kernel_cannot_record_is_refused_before_anything_changes() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let queued = database
        .submit_job(&publish(&collection("queued")), at(0))
        .unwrap();
    let created = journaled(&database, queued.id);
    let before_1970 = UNIX_EPOCH - Duration::from_millis(1);
    let after_9999 = UNIX_EPOCH + Duration::from_secs(253_402_300_800);
    let last_moment = after_9999 - Duration::from_millis(1);
    let cases = [
        (before_1970, TERM),
        (last_moment, TERM),
        (at(0), Duration::MAX),
    ];
    for (now, term) in cases {
        let refused = database.take_job(queued.id, FIRST, now, term).unwrap_err();
        assert!(
            matches!(refused, Error::Time),
            "{now:?} {term:?}: {refused:?}"
        );
    }
    assert_eq!(
        database
            .job(&ScopeSet::default_workspace(), queued.id)
            .unwrap()
            .unwrap()
            .state,
        JobState::Queued
    );
    assert_eq!(journaled(&database, queued.id), created);
    let (job, mut lease) = running(&database, "running");
    let before = database
        .job(&ScopeSet::default_workspace(), job.id)
        .unwrap();
    let journal = journaled(&database, job.id);
    let refusals = [
        database
            .heartbeat(&mut lease, before_1970, TERM)
            .unwrap_err(),
        database
            .progress(&mut lease, at(1), Duration::MAX, &json!({"step": 1}))
            .unwrap_err(),
    ];
    for refused in refusals {
        assert!(matches!(refused, Error::Time), "{refused:?}");
    }
    assert_eq!(
        database
            .job(&ScopeSet::default_workspace(), job.id)
            .unwrap(),
        before
    );
    assert_eq!(lease.heartbeat, "2026-09-26T12:00:00.000Z");
    assert_eq!(journaled(&database, job.id), journal);
    database
        .heartbeat(&mut lease, last_moment, Duration::ZERO)
        .unwrap();
    assert_eq!(lease.expires, "9999-12-31T23:59:59.999Z");
}
