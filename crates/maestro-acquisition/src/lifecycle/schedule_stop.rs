//! Owner-fenced schedule stop and recovery on the existing kernel jobs.
use crate::{Refusal, policy::format_time};
use maestro_kernel::{
    job::{CREATED, Job, JobState, Lease, stream},
    journal::{Filter, NewEvent},
    scope::ScopeSet,
    store::Database,
};
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ulid::Ulid;

/// Existing kernel schedule job kind, separate from a source writer.
pub const JOB_KIND: &str = "acquisition.schedule";
/// Durable owner stop on the existing job stream.
const STOP_REQUESTED: &str = "maestro.acquisition.schedule.stop_requested.v1";

/// Verify the exact visible live schedule and its frozen local owner.
/// # Errors
/// Missing, terminal, wrong-kind, foreign-owner or denied jobs refuse.
pub(super) fn owned(
    db: &Database,
    scopes: &ScopeSet,
    id: Ulid,
    principal: &str,
) -> Result<Job, Refusal> {
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

/// Millisecond UTC spelling, shared by expiry checks and activation validation.
/// Kernel jobs additionally require times at or after the Unix epoch.
pub(super) fn wall_time(wall: SystemTime) -> Result<String, Refusal> {
    let fraction = wall
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Refusal::Invalid)?
        .subsec_millis();
    Ok(format!(
        "{}.{fraction:03}Z",
        format_time(wall)?.trim_end_matches('Z')
    ))
}
/// An explicit activation may retire only an expired, stopped lease of its owner.
/// Taking and completing fence any competing renewal or takeover; no sync is resumed.
pub(super) fn recover(
    db: &Database,
    scopes: &ScopeSet,
    resource: &str,
    principal: &str,
    timing: (SystemTime, Duration),
) -> Result<(), Refusal> {
    let (wall, term) = timing;
    for job in db
        .unfinished_jobs(scopes, resource)
        .map_err(|_| Refusal::Access)?
    {
        owned(db, scopes, job.id, principal)?;
        let lease = job.lease.ok_or(Refusal::Access)?;
        if lease.holder != principal || !stop_requested(db, scopes, job.id)? {
            return Err(Refusal::Access);
        }
        let lease = db
            .take_job(job.id, principal, wall, term)
            .map_err(|_| Refusal::Access)?;
        db.complete_job(&lease, JobState::Cancelled, &json!({"reason":"owner_stop"}))
            .map_err(|_| Refusal::Access)?;
    }
    Ok(())
}
/// Take only this activation's freshly submitted reservation; failed takes must free it.
/// Atomic queued-only cancellation refuses if a competitor already took the job.
pub(super) fn take_reserved(
    db: &Database,
    id: Ulid,
    principal: &str,
    wall: SystemTime,
    term: Duration,
) -> Result<Lease, Refusal> {
    if let Ok(lease) = db.take_job(id, principal, wall, term) {
        Ok(lease)
    } else {
        // A running competitor must never be completed with its saved lease.
        db.cancel_job(id, &json!({"reason":"activation_lease_failed"}))
            .map_err(|_| Refusal::Access)?;
        Err(Refusal::Access)
    }
}

#[cfg(test)]
mod tests {
    use super::{JOB_KIND, take_reserved};
    use maestro_kernel::{
        job::{JobState, NewJob},
        scope::Right,
        store::Database,
    };
    use maestro_test_scratch::scratch_directory;
    use serde_json::json;
    use std::{
        fs,
        time::{Duration, UNIX_EPOCH},
    };

    #[test]
    fn n42_post_submission_take_failure_cleans_only_own_queued_job() {
        let root = scratch_directory().unwrap();
        let db = Database::open_in(&root).unwrap();
        let scope = "workspace/default/collection/synthetic".parse().unwrap();
        db.grant("owner", &scope, Right::Read, "test").unwrap();
        let scopes = db.visible("owner").unwrap();
        let wall = UNIX_EPOCH + Duration::from_secs(2_000_000);
        for competing in [false, true] {
            let job = db
                .submit_job(
                    &NewJob {
                        kind: JOB_KIND,
                        inputs: &json!({"owner":"owner","competing":competing}),
                        scope: &scope,
                        resource: Some("schedule/test"),
                    },
                    wall,
                )
                .unwrap();
            if competing {
                db.take_job(job.id, "competitor", wall, Duration::from_secs(10))
                    .unwrap();
            }
            let before = db.job(&scopes, job.id).unwrap().unwrap();
            assert!(
                take_reserved(
                    &db,
                    job.id,
                    "owner",
                    UNIX_EPOCH + Duration::from_secs(253_402_300_799),
                    Duration::from_secs(10)
                )
                .is_err()
            );
            let after = db.job(&scopes, job.id).unwrap().unwrap();
            if competing {
                assert_eq!(after, before);
            } else {
                assert_eq!(after.state, JobState::Cancelled);
                assert!(
                    db.unfinished_jobs(&scopes, "schedule/test")
                        .unwrap()
                        .is_empty()
                );
            }
        }
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
}
