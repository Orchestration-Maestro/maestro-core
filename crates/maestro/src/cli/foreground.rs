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
    lease::{Holder, TIMING, Timing, holder},
    output::Output,
    wait::{self, Follower, POLL, Printing},
};
use crate::{failure::Failure, kernel::Kernel};
#[cfg(feature = "engine")]
use maestro_kernel::job::Submitted;
use maestro_kernel::{
    job::{self, Job, JobState, Lease, NewJob},
    store::Database,
};
use serde_json::{Value, json};
use std::{process::ExitCode, thread, time::SystemTime};
use ulid::Ulid;

/// Submission choice; existing foreground callers always reuse their key.
pub(super) enum Policy {
    /// Preserve the existing retry/supersede behavior.
    Reuse,
    /// Never take over or supersede a pre-existing attempt.
    #[cfg(feature = "engine")]
    FreshOnly,
    /// Take exactly this already validated job, without submission.
    #[cfg(feature = "engine")]
    Named(Ulid),
}

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
    run_selected(kernel, output, (new, Policy::Reuse), timing, work).map(|(job, _)| job)
}

/// Run the private submission policy; the boolean distinguishes a fresh
/// execution from an already succeeded attempt requiring receipt validation.
pub(super) fn run_selected(
    kernel: &Kernel,
    output: Output,
    request: (&NewJob<'_>, Policy),
    timing: Timing,
    work: impl FnOnce(&Holder<'_>) -> (JobState, Value),
) -> Result<(Job, bool), Failure> {
    let (new, policy) = request;
    let (job, fresh) = match policy {
        Policy::Reuse => (submit(&kernel.database, new)?, true),
        #[cfg(feature = "engine")]
        Policy::Named(id) => (
            kernel
                .database
                .job(&kernel.scopes, id)
                .map_err(|error| Failure::refused_by(&error))?
                .ok_or_else(|| Failure::refused("unknown or unauthorized resume job"))?,
            true,
        ),
        #[cfg(feature = "engine")]
        Policy::FreshOnly => match kernel.database.submit_new_job(new, SystemTime::now()) {
            Ok(Submitted::Created(job)) => (job, true),
            Ok(Submitted::Found(job)) if job.state == JobState::Succeeded => (job, false),
            Ok(Submitted::Found(job)) => {
                return Err(Failure::refused(format!(
                    "an unfinished build exists; repeat with --resume {}",
                    job.id
                )));
            }
            Err(error @ job::Error::ResourceHeld { .. }) => {
                return Err(Failure::refused_by(&error));
            }
            Err(error) => return Err(Failure::failed_by(&error)),
        },
    };

    output.job(job.id)?;
    if !fresh {
        return Ok((job, false));
    }
    let mut follower = Follower::new(kernel, output, job.id);
    loop {
        if let Some(lease) = take(&kernel.database, job.id, timing)? {
            return Holder::run(&kernel.database, lease, timing, work)
                .map(|job| (job, true))
                .map_err(|error| Failure::failed_by(&error));
        }
        for _ in 0..LOOKS_PER_TRY {
            if let Some(ended) = follower.look()? {
                return Ok((ended, true));
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
