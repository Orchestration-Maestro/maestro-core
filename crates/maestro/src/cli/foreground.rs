//! A job run in the foreground (plan D5, T016), as `knowledge import` and
//! `knowledge quality` run theirs: submitted with its key, or found by it;
//! its ID printed first; its lease taken for this process and its work run
//! under it, or, while another process holds it, the job followed, its lease
//! tried about once a second, so that it is taken over once it expired, as
//! it is taken over at once when it expired before; and the job printed as
//! it ended. The loop holds the lease itself, through `Holder::run`, whose
//! heartbeats renew it while the work runs: a command hands over its work,
//! never the lease, so no work runs without them. A job of the same kind and
//! other inputs that holds the resource, whose lease expired or that no
//! process took, is superseded: cancelled, then this job submitted again,
//! once. One a live lease holds refuses this job, naming it.

use super::{
    failure::Failure,
    kernel::Kernel,
    lease::{Holder, TIMING, Timing, holder},
    output::Output,
    wait::{self, Follower, POLL, Printing},
};
use maestro_kernel::{
    job::{self, Job, JobState, Lease, NewJob},
    store::Database,
};
use serde_json::{Value, json};
use std::{process::ExitCode, thread, time::SystemTime};
use ulid::Ulid;

/// How many looks at a job another process holds pass between two tries of
/// its lease: about a second, at one look every [`POLL`].
const LOOKS_PER_TRY: u32 = 10;

/// Runs the job `new` describes in the foreground, under the command line's
/// leases, [`TIMING`], as [`run_to_end`] does, then prints it as it ended,
/// as `printing` says, and returns the exit code of its outcome.
///
/// # Errors
///
/// As [`run_to_end`].
pub(super) fn run(
    kernel: &Kernel,
    output: Output,
    new: &NewJob<'_>,
    work: impl FnOnce(&Holder<'_>) -> (JobState, Value),
    printing: Printing,
) -> Result<ExitCode, Failure> {
    let ended = run_to_end(kernel, output, new, TIMING, work)?;
    wait::report(output, printing, &ended)
}

/// Runs the job `new` describes to its end, or follows it there, printing
/// its ID first, and returns it as it ended: `work` runs it under the lease
/// this process takes for the term of `timing`, which heartbeats renew at
/// each beat of `timing` while it runs, and the job ends in the state `work`
/// returns, with its outcome; or the job another process holds is followed
/// to its end.
///
/// # Errors
///
/// [`Failure::Refused`], naming the job, when a live lease holds another
/// job on the resource of `new`, and [`Failure::Failed`] when the kernel
/// fails, a lease another process took over while `work` ran among the
/// reasons.
pub(super) fn run_to_end(
    kernel: &Kernel,
    output: Output,
    new: &NewJob<'_>,
    timing: Timing,
    work: impl FnOnce(&Holder<'_>) -> (JobState, Value),
) -> Result<Job, Failure> {
    let job = submit(&kernel.database, new)?;
    output.job(job.id)?;
    let mut follower = Follower::new(kernel, output, job.id);
    loop {
        if let Some(lease) = take(&kernel.database, job.id, timing)? {
            return Holder::run(&kernel.database, lease, timing, work)
                .map_err(|error| Failure::failed_by(&error));
        }
        for _ in 0..LOOKS_PER_TRY {
            if let Some(ended) = follower.look()? {
                return Ok(ended);
            }
            thread::sleep(POLL);
        }
    }
}

/// Submits `new` in `database`, or finds the job of its key. When another
/// job holds its resource, that job is superseded if it may be, and `new`
/// submitted again, once.
///
/// # Errors
///
/// [`Failure::Refused`], naming the job, when a live lease holds the
/// resource's job, or another job holds it again; [`Failure::Failed`] when
/// the kernel fails.
fn submit(database: &Database, new: &NewJob<'_>) -> Result<Job, Failure> {
    let mut submitted = database.submit_job(new, SystemTime::now());
    if let Err(job::Error::ResourceHeld { job: holding, .. }) = submitted {
        let free =
            supersede(database, holding, new.inputs).map_err(|error| Failure::failed_by(&error))?;
        if free {
            submitted = database.submit_job(new, SystemTime::now());
        }
    }
    submitted.map_err(|error| match error {
        job::Error::ResourceHeld { .. } => Failure::refused_by(&error),
        _ => Failure::failed_by(&error),
    })
}

/// Supersedes `holding`, the job that holds the resource a job of `inputs`
/// needs, unless a live lease holds it: its lease is taken for this process,
/// and it is cancelled with `{"superseded_by": {"inputs": inputs}}`. Returns
/// whether the resource is free now: after that, or when the job ended
/// meanwhile, and not while a live lease holds it.
///
/// # Errors
///
/// The kernel's failures.
pub(super) fn supersede(
    database: &Database,
    holding: Ulid,
    inputs: &Value,
) -> Result<bool, job::Error> {
    let lease = match database.take_job(holding, &holder(), SystemTime::now(), TIMING.term) {
        Ok(lease) => lease,
        Err(job::Error::Held { .. }) => return Ok(false),
        Err(job::Error::IllegalMove { .. }) => return Ok(true),
        Err(error) => return Err(error),
    };
    let outcome = json!({ "superseded_by": { "inputs": inputs } });
    database.complete_job(&lease, JobState::Cancelled, &outcome)?;
    Ok(true)
}

/// The lease of job `id` in `database`, taken for this process for the term
/// of `timing`; none while another process holds a live lease, or once the
/// job ended.
///
/// # Errors
///
/// [`Failure::Failed`] when the kernel fails.
fn take(database: &Database, id: Ulid, timing: Timing) -> Result<Option<Lease>, Failure> {
    match database.take_job(id, &holder(), SystemTime::now(), timing.term) {
        Ok(lease) => Ok(Some(lease)),
        // The follower's next look finds a job that ended.
        Err(job::Error::Held { .. } | job::Error::IllegalMove { .. }) => Ok(None),
        Err(error) => Err(Failure::failed_by(&error)),
    }
}
