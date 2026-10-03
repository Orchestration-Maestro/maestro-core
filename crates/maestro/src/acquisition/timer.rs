//! Owner stop is durable kernel state, never PID signalling or a timeout-only success.
use super::output::Report;
use crate::{failure::Failure, kernel::Kernel};
use maestro_acquisition::lifecycle::schedule::request_stop;
use maestro_kernel::{
    acquisition::Status,
    job::JobState,
    paths::{Environment, data_dir},
    scope::LOCAL,
};
use std::{
    thread,
    time::{Duration, Instant},
};
use ulid::Ulid;

/// Live recurring activation has no separately granted effect in N05 yet.
pub(crate) fn activate() -> Result<Report, Failure> {
    Err(Failure::refused(concat!(
        "live timer activation refused: N05 grants carry only Fetch and ",
        "RobotsOverride; no active-source activation effect exists yet; ",
        "obtain its exact OA4a grant after that contract is implemented"
    )))
}
/// Record owner stop and wait for the adapter's durable cancellation, never kill a PID.
/// The CLI deadline mirrors Pi fast tools (300000 ms, maximum 2147483647 ms).
pub(crate) fn stop(id: Ulid, deadline_ms: u64) -> Result<Report, Failure> {
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(deadline_ms))
        .ok_or_else(|| Failure::refused("acquisition stop deadline invalid"))?;
    let data = data_dir(&Environment::current()).map_err(|error| Failure::failed_by(&error))?;
    if !data
        .join("kernel.sqlite3")
        .try_exists()
        .map_err(|error| Failure::failed_by(&error))?
    {
        return Err(unavailable());
    }
    let kernel = Kernel::open()?;
    check_deadline(deadline)?;
    request_stop(&kernel.database, &kernel.scopes, id, LOCAL).map_err(|_| unavailable())?;
    loop {
        check_deadline(deadline)?;
        let scopes = kernel
            .database
            .visible(LOCAL)
            .map_err(|error| Failure::failed_by(&error))?;
        let job = kernel
            .database
            .job(&scopes, id)
            .map_err(|error| Failure::failed_by(&error))?
            .ok_or_else(unavailable)?;
        check_deadline(deadline)?;
        if job.state == JobState::Cancelled {
            let mut report = Report::new();
            report.status = Status::Complete;
            return Ok(report);
        }
        // This is an observation interval, not a shutdown allowance; the deadline wins.
        thread::sleep(
            Duration::from_millis(1).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
/// A timeout retains the durable stop request; it is not cancellation acknowledgement.
fn check_deadline(deadline: Instant) -> Result<(), Failure> {
    if Instant::now() >= deadline {
        return Err(Failure::refused(concat!(
            "acquisition stop deadline elapsed; stop request may be durable but ",
            "cancellation is not acknowledged; retry stop with a longer --deadline-ms"
        )));
    }
    Ok(())
}
/// No foreign metadata or process identity is disclosed by stop refusals.
fn unavailable() -> Failure {
    Failure::refused(concat!(
        "no owned live acquisition schedule for this job; ",
        "use the exact job ID returned by an explicit owned activation"
    ))
}

#[cfg(test)]
mod tests {
    use super::super::flow_fixture::{Fixture, clean};
    use maestro_acquisition::{
        Refusal,
        lifecycle::{
            full::Mode,
            schedule::{Activation, LocalTimer, ScheduleTrigger, SyncRequest, TriggerKind},
        },
        policy::source::SyncMode,
    };
    use maestro_kernel::acquisition::{Handle, Status};
    use std::{
        env,
        num::NonZeroU64,
        process::{Child, Command},
        sync::mpsc,
        time::{Duration, Instant, SystemTime},
    };

    /// The identical production sync, with synthetic site/authority ports only.
    struct Admitted<'a>(&'a Fixture);
    impl ScheduleTrigger for Admitted<'_> {
        fn invoke(&mut self, request: &SyncRequest, principal: &str) -> Result<Handle, Refusal> {
            assert_eq!(principal, "reader");
            let report = self.0.sync_window(request.mode, SystemTime::now());
            assert_eq!(report.status, Status::Complete, "{report:?}");
            assert!(!report.completed.is_empty());
            report.run.ok_or(Refusal::Invalid)
        }
    }

    #[test]
    fn n42_real_timer_process_uses_the_same_admitted_sync() {
        const CHILD: &str = "MAESTRO_N42_ADMITTED_CHILD";
        if env::var_os(CHILD).is_none() {
            let mut child = Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "acquisition::timer::tests::n42_real_timer_process_uses_the_same_admitted_sync",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .spawn()
                .unwrap();
            wait_owned(&mut child);
            return;
        }
        let fixture = Fixture::new(clean);
        let mut policy = fixture.policy.policy().sources[0].sync.clone();
        policy.mode = SyncMode::OneOff;
        let mut timer = LocalTimer::activate(
            &fixture.db,
            &Activation {
                policy: &policy,
                scope: &fixture.scope,
                principal: "reader",
                mode: Mode::Full,
                lease_term: Duration::from_secs(15),
            },
            Instant::now(),
            SystemTime::now(),
        )
        .unwrap();
        let mut trigger = Admitted(&fixture);
        let (_wake, timer_signal) = mpsc::channel::<()>();
        assert!(timer_signal.recv_timeout(Duration::from_millis(1)).is_err());
        assert!(
            timer
                .poll(&mut trigger, Instant::now(), SystemTime::now())
                .unwrap()
                .is_some()
        );
        let requests = fixture.site.requests.lock().unwrap().len();
        assert!(requests > 0);
        let duplicate = timer.request(TriggerKind::Timer);
        assert!(
            timer
                .invoke(&mut trigger, &duplicate, Instant::now(), SystemTime::now())
                .is_err()
        );
        assert_eq!(fixture.site.requests.lock().unwrap().len(), requests);
        drop(timer);
        policy.mode = SyncMode::Watch;
        policy.timer_period_ms = NonZeroU64::new(86_400_000);
        let epoch = Instant::now();
        let mut timer = LocalTimer::activate(
            &fixture.db,
            &Activation {
                policy: &policy,
                scope: &fixture.scope,
                principal: "reader",
                mode: Mode::Incremental,
                lease_term: Duration::from_hours(48),
            },
            epoch,
            SystemTime::now(),
        )
        .unwrap();
        assert!(
            timer
                .poll(&mut trigger, epoch, SystemTime::now())
                .unwrap()
                .is_none()
        );
        assert!(
            timer
                .poll(
                    &mut trigger,
                    epoch + Duration::from_hours(24),
                    SystemTime::now()
                )
                .unwrap()
                .is_some()
        );
        drop(timer);
        fixture.finish();
    }

    /// Bound only the child handle this test owns; no process-name or PID search.
    fn wait_owned(child: &mut Child) {
        let (_wake, timer_signal) = mpsc::channel::<()>();
        let deadline = Instant::now() + Duration::from_secs(15);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("owned timer process did not finish");
            }
            assert!(timer_signal.recv_timeout(Duration::from_millis(1)).is_err());
        }
        assert!(child.wait().unwrap().success());
    }
}
