//! One-shot qualification evidence authenticated by Linux peer credentials.
use crate::failure::Failure;
use maestro_acquisition::policy::authority_socket::{frame_timeout, read_frame_with_clock};
use maestro_kernel::retrieval::Clock;
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    net::sockopt::socket_peercred,
    process::{Pid, PidfdFlags, pidfd_open},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write as _},
    os::{
        fd::AsFd as _,
        unix::{
            fs::PermissionsExt as _,
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// All mutation outcomes must come from the authenticated probe, not its launcher.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Report {
    /// Creating an authority store file was denied.
    pub(super) create_denied: bool,
    /// Editing the existing canary was denied.
    pub(super) edit_denied: bool,
    /// Deleting the existing canary was denied.
    pub(super) delete_denied: bool,
}
/// Inputs pinned by the host qualifier, not by a source manifest.
pub(super) struct Probe<'a> {
    /// Admin-provisioned launcher.
    pub(super) launcher: &'a Path,
    /// Exact digest-checked executable.
    pub(super) binary: &'a Path,
    /// Protected store to probe.
    pub(super) store: &'a Path,
    /// One-shot endpoint in the protected socket parent.
    pub(super) endpoint: PathBuf,
    /// Exact kernel UID required for this invocation.
    pub(super) uid: u32,
}
/// Remove only an endpoint this invocation successfully created.
struct Endpoint(PathBuf);
impl Drop for Endpoint {
    fn drop(&mut self) {
        drop(fs::remove_file(&self.0));
    }
}
/// Kill and reap even when authentication, framing or the deadline refuses.
struct Launcher(Child);
impl Drop for Launcher {
    fn drop(&mut self) {
        drop(self.0.kill());
        drop(self.0.wait());
    }
}
/// Fixed refusal, without launcher or source content.
fn refused() -> Failure {
    Failure::refused("authority unqualified")
}
/// Positive cumulative budget for poll, using the qualifier's replaceable clock.
fn timeout(deadline: Instant, clock: &dyn Clock) -> Result<Timespec, Failure> {
    frame_timeout(deadline, clock.now())
        .map_err(|_| refused())?
        .try_into()
        .map_err(|_| refused())
}
/// Authenticate a single completion and require successful launcher exit in one budget.
pub(super) fn qualify(
    probe: &Probe<'_>,
    budget: Duration,
    clock: &dyn Clock,
) -> Result<(), Failure> {
    let listener = UnixListener::bind(&probe.endpoint).map_err(|_| refused())?;
    let _endpoint = Endpoint(probe.endpoint.clone());
    fs::set_permissions(&probe.endpoint, fs::Permissions::from_mode(0o666))
        .map_err(|_| refused())?;
    let deadline = clock.now() + budget;
    // Launcher stdout is diagnostic only; never mix it into CLI JSON or evidence.
    let diagnostics = io::stderr()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|_| refused())?;
    let mut child = Launcher(
        Command::new(probe.launcher)
            .arg(probe.uid.to_string())
            .arg(probe.binary)
            .args(["authority", "probe-store", "--store"])
            .arg(probe.store)
            .arg("--qualification-socket")
            .arg(&probe.endpoint)
            .stdout(Stdio::from(diagnostics))
            .spawn()
            .map_err(|_| refused())?,
    );
    let pid = Pid::from_raw(child.0.id().try_into().map_err(|_| refused())?).ok_or_else(refused)?;
    let pidfd = pidfd_open(pid, PidfdFlags::empty()).map_err(|_| refused())?;
    let mut pending = [
        PollFd::new(&listener, PollFlags::IN),
        PollFd::new(&pidfd, PollFlags::IN),
    ];
    poll(&mut pending, Some(&timeout(deadline, clock)?)).map_err(|_| refused())?;
    timeout(deadline, clock)?;
    if !pending[0].revents().contains(PollFlags::IN) {
        return Err(refused());
    }
    let (mut stream, _) = listener.accept().map_err(|_| refused())?;
    authenticated_report(&mut stream, probe.uid, clock)?;
    timeout(deadline, clock)?;
    let mut exit = [PollFd::new(&pidfd, PollFlags::IN)];
    poll(&mut exit, Some(&timeout(deadline, clock)?)).map_err(|_| refused())?;
    timeout(deadline, clock)?;
    if !exit[0].revents().contains(PollFlags::IN)
        || !child.0.wait().map_err(|_| refused())?.success()
    {
        return Err(refused());
    }
    // A second completion is never ignored, including one queued with child exit.
    let mut extra = [PollFd::new(&listener, PollFlags::IN)];
    if poll(&mut extra, Some(&Timespec::default())).map_err(|_| refused())? != 0 {
        return Err(refused());
    }
    Ok(())
}
/// Require the actual expected UID and all strict denied outcomes on this descriptor.
fn authenticated_report(
    stream: &mut UnixStream,
    uid: u32,
    clock: &dyn Clock,
) -> Result<(), Failure> {
    if socket_peercred(&*stream)
        .map_err(|_| refused())?
        .uid
        .as_raw()
        != uid
    {
        return Err(refused());
    }
    let report: Report =
        serde_json::from_slice(&read_frame_with_clock(stream, clock).map_err(|_| refused())?)
            .map_err(|_| refused())?;
    if !report.create_denied || !report.edit_denied || !report.delete_denied {
        return Err(refused());
    }
    Ok(())
}
/// Deliver denied mutation outcomes on the authenticated one-shot channel.
pub(super) fn send(stream: &mut UnixStream, report: &Report) -> Result<(), Failure> {
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| refused())?;
    writeln!(
        stream,
        "{}",
        serde_json::to_string(report).map_err(|_| refused())?
    )
    .map_err(|_| refused())
}

#[cfg(test)]
mod tests {
    use super::{Clock, Duration, Instant, UnixStream, authenticated_report, timeout};
    use rustix::process::geteuid;
    use std::io::Write as _;

    /// No wall-clock progress or sleep is needed to exercise queued authentication.
    #[derive(Debug)]
    struct StoppedClock(Instant);
    impl Clock for StoppedClock {
        fn now(&self) -> Instant {
            self.0
        }
    }
    #[test]
    fn n05_qualification_report_requires_kernel_uid_and_strict_denials() {
        let clock = StoppedClock(Instant::now());
        let uid = geteuid().as_raw();
        let valid = r#"{"create_denied":true,"edit_denied":true,"delete_denied":true}"#;
        for (peer, report, accepted) in [
            (uid, valid.to_owned(), true),
            (uid + 1, valid.to_owned(), false),
            (uid, valid.replacen("true", "false", 1), false),
            (
                uid,
                valid.replace("edit_denied\":true", "edit_denied\":false"),
                false,
            ),
            (
                uid,
                valid.replace("delete_denied\":true", "delete_denied\":false"),
                false,
            ),
            (uid, valid.replace('}', ",\"approval\":true}"), false),
            (uid, "{\"create_denied\":true}".into(), false),
        ] {
            let (mut reader, mut writer) = UnixStream::pair().unwrap();
            writeln!(writer, "{report}").unwrap();
            assert_eq!(
                authenticated_report(&mut reader, peer, &clock).is_ok(),
                accepted,
                "{report}"
            );
        }
        assert!(timeout(clock.0 + Duration::from_secs(15), &clock).is_ok());
        assert!(timeout(clock.0, &clock).is_err());
    }
}
