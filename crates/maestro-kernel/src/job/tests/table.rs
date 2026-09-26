//! The jobs table: what the database itself refuses, whoever writes, so no
//! program outside the kernel breaks what the kernel relies on.

use super::support::{SECOND, Scratch, TERM, at, collection, publish, running};
use crate::job::JobState;
use rusqlite::Connection;
use serde_json::json;

/// A lease time as the kernel writes it, as SQL.
const TIME: &str = "'2026-09-26T12:00:00.000Z'";

/// Inserts through `outside` the job of `values`: its ID, kind, key, attempt,
/// scope, resource, state, lease number, holder, heartbeat, expiry and
/// outcome, as SQL, where `{t}` stands for [`TIME`].
fn insert(outside: &Connection, values: &str) -> rusqlite::Result<usize> {
    outside.execute(
        &format!(
            "INSERT INTO jobs (id, kind, idempotency_key, attempt, scope, resource, state,
               lease_number, lease_holder, lease_heartbeat, lease_expires, outcome_json)
             VALUES ({})",
            values.replace("{t}", TIME)
        ),
        [],
    )
}

/// The message of the error `insert` gives for `values`.
fn refusal(outside: &Connection, values: &str) -> String {
    insert(outside, values).unwrap_err().to_string()
}

#[test]
fn the_table_refuses_what_no_job_is() {
    let scratch = Scratch::new();
    drop(scratch.open());
    let outside = scratch.outside();
    let broken = [
        "'a', 'k', 'key', 1, 's', NULL, 'paused', 0, NULL, NULL, NULL, NULL",
        "'b', 'k', 'key', 0, 's', NULL, 'queued', 0, NULL, NULL, NULL, NULL",
        "'c', 'k', 'key', 1, 's', NULL, 'running', 1, NULL, NULL, NULL, NULL",
        "'d', 'k', 'key', 1, 's', NULL, 'queued', 1, 'h', {t}, {t}, NULL",
        "'e', 'k', 'key', 1, 's', NULL, 'running', 1, 'h', NULL, {t}, NULL",
        "'f', 'k', 'key', 1, 's', NULL, 'running', 1, 'h', {t}, NULL, NULL",
        "'g', 'k', 'key', 1, 's', NULL, 'running', 0, 'h', {t}, {t}, NULL",
        "'h', 'k', 'key', 1, 's', NULL, 'succeeded', 1, NULL, NULL, NULL, NULL",
        "'i', 'k', 'key', 1, 's', NULL, 'running', 1, 'h', {t}, {t}, '{}'",
        "'j', 'k', 'key', 1, 's', NULL, 'failed', 1, NULL, NULL, NULL, 'not json'",
        "'k', 'k', 'key', 1, 's', NULL, 'queued', -1, NULL, NULL, NULL, NULL",
    ];
    for values in broken {
        let error = refusal(&outside, values);
        assert!(
            error.starts_with("CHECK constraint failed"),
            "{values}: {error}"
        );
    }
    insert(
        &outside,
        "'l', 'k', 'key', 1, 's', NULL, 'failed', 1, NULL, NULL, NULL, '{}'",
    )
    .unwrap();
    insert(
        &outside,
        "'m', 'k', 'key', 2, 's', NULL, 'running', 1, 'h', {t}, {t}, NULL",
    )
    .unwrap();
    let live = [
        "'n', 'k', 'key', 3, 's', NULL, 'queued', 0, NULL, NULL, NULL, NULL",
        "'o', 'k', 'key', 3, 's', NULL, 'succeeded', 1, NULL, NULL, NULL, '{}'",
        "'p', 'k', 'key', 2, 's', NULL, 'cancelled', 0, NULL, NULL, NULL, '{}'",
    ];
    for values in live {
        let error = refusal(&outside, values);
        assert!(
            error.starts_with("UNIQUE constraint failed: jobs.idempotency_key"),
            "{values}: {error}"
        );
    }
    insert(
        &outside,
        "'q', 'k', 'key', 3, 's', NULL, 'cancelled', 0, NULL, NULL, NULL, '{}'",
    )
    .unwrap();
}

#[test]
fn a_lease_time_is_rfc_3339_in_utc_to_the_millisecond_whoever_writes() {
    let scratch = Scratch::new();
    drop(scratch.open());
    let outside = scratch.outside();
    let times = [
        ("'t'", TIME),
        (TIME, "'t'"),
        (TIME, "'2026-09-26 12:00:30'"),
        (TIME, "'2026-09-26T12:00:30Z'"),
        (TIME, "'2026-09-26T17:00:30.000+05:00'"),
        (TIME, "'2026-09-26T12:00:30.000Z '"),
    ];
    for (heartbeat, expires) in times {
        let values = format!(
            "'a', 'k', 'key', 1, 's', NULL, 'running', 1, 'h', {heartbeat}, {expires}, NULL"
        );
        let error = refusal(&outside, &values);
        assert!(
            error.starts_with("CHECK constraint failed: lease_"),
            "{values}: {error}"
        );
    }
    insert(
        &outside,
        "'b', 'k', 'key', 1, 's', NULL, 'running', 1, 'h', {t}, '9999-12-31T23:59:59.999Z', NULL",
    )
    .unwrap();
}

#[test]
fn one_queued_or_running_job_at_most_holds_a_resource_whoever_writes() {
    let scratch = Scratch::new();
    drop(scratch.open());
    let outside = scratch.outside();
    insert(
        &outside,
        "'a', 'k', 'key-a', 1, 's', 'res', 'queued', 0, NULL, NULL, NULL, NULL",
    )
    .unwrap();
    let holding = [
        "'b', 'k', 'key-b', 1, 's', 'res', 'queued', 0, NULL, NULL, NULL, NULL",
        "'c', 'k', 'key-c', 1, 's', 'res', 'running', 1, 'h', {t}, {t}, NULL",
    ];
    for values in holding {
        let error = refusal(&outside, values);
        assert!(
            error.starts_with("UNIQUE constraint failed: jobs.resource"),
            "{values}: {error}"
        );
    }
    let ended = [
        "'d', 'k', 'key-d', 1, 's', 'res', 'succeeded', 1, NULL, NULL, NULL, '{}'",
        "'e', 'k', 'key-e', 1, 's', 'res', 'failed', 1, NULL, NULL, NULL, '{}'",
        "'f', 'k', 'key-f', 1, 's', 'res', 'cancelled', 0, NULL, NULL, NULL, '{}'",
        "'g', 'k', 'key-g', 1, 's', 'other', 'queued', 0, NULL, NULL, NULL, NULL",
        "'h', 'k', 'key-h', 1, 's', NULL, 'queued', 0, NULL, NULL, NULL, NULL",
        "'i', 'k', 'key-i', 1, 's', NULL, 'queued', 0, NULL, NULL, NULL, NULL",
    ];
    for values in ended {
        insert(&outside, values).unwrap();
    }
}

#[test]
fn a_job_that_ended_never_changes_whoever_writes() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, lease) = running(&database, "demo");
    let ended = database
        .complete_job(&lease, JobState::Failed, &json!({"reason": "down"}))
        .unwrap();
    let outside = scratch.outside();
    let changes = [
        "state = 'queued', outcome_json = NULL",
        "outcome_json = '{}'",
        "kind = 'knowledge.prepare'",
        "lease_number = lease_number + 1",
    ];
    for change in changes {
        let error = outside
            .execute(
                &format!("UPDATE jobs SET {change} WHERE id = ?1"),
                [job.id.to_string()],
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with("a job that ended never changes"),
            "{change}: {error}"
        );
    }
    assert_eq!(database.job(job.id).unwrap(), Some(ended));
}

#[test]
fn a_job_moves_only_forward_whoever_writes() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let queued = database
        .submit_job(&publish(&collection("queued")), at(0))
        .unwrap();
    let (running, _) = running(&database, "running");
    let outside = scratch.outside();
    let moves = [
        (queued.id, "state = 'succeeded', outcome_json = '{}'"),
        (queued.id, "state = 'failed', outcome_json = '{}'"),
        (
            running.id,
            "state = 'queued', lease_holder = NULL, lease_heartbeat = NULL, lease_expires = NULL",
        ),
    ];
    for (id, change) in moves {
        let before = database.job(id).unwrap();
        let error = outside
            .execute(
                &format!("UPDATE jobs SET {change} WHERE id = ?1"),
                [id.to_string()],
            )
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("a job moves only"), "{change}: {error}");
        assert_eq!(database.job(id).unwrap(), before);
    }
}

#[test]
fn a_lease_number_never_decreases_whoever_writes() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let (job, _) = running(&database, "demo");
    let taken = database.take_job(job.id, SECOND, at(30), TERM).unwrap();
    let error = scratch
        .outside()
        .execute(
            "UPDATE jobs SET lease_number = 1 WHERE id = ?1",
            [job.id.to_string()],
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.starts_with("a lease number never decreases"),
        "{error}"
    );
    assert_eq!(database.job(job.id).unwrap().unwrap().lease, Some(taken));
}
