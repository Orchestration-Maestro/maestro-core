//! One-shot qualification evidence authenticated by Linux peer credentials.
use crate::failure::Failure;
use maestro_acquisition::{
    lifecycle::resume::stop_owned,
    policy::authority_socket::{frame_timeout, read_frame_with_clock},
};
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
struct Launcher<'a> {
    /// Only the child spawned by this invocation.
    child: Child,
    /// The qualifier's trusted clock also bounds cancellation acknowledgement.
    clock: &'a dyn Clock,
}
impl Launcher<'_> {
    /// Cancellation never treats a client timeout as a successful child exit.
    fn stop(&mut self) -> Result<(), Failure> {
        stop_owned(
            &mut self.child,
            self.clock.now() + Duration::from_secs(2),
            self.clock,
        )
        .map(|_| ())
        .map_err(|_| refused())
    }
}
impl Drop for Launcher<'_> {
    fn drop(&mut self) {
        drop(self.stop());
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
    let mut child = Launcher {
        child: Command::new(probe.launcher)
            .arg(probe.uid.to_string())
            .arg(probe.binary)
            .args(["authority", "probe-store", "--store"])
            .arg(probe.store)
            .arg("--qualification-socket")
            .arg(&probe.endpoint)
            .stdout(Stdio::from(diagnostics))
            .spawn()
            .map_err(|_| refused())?,
        clock,
    };
    let result = completion(probe, &listener, &mut child, deadline, clock);
    child.stop()?;
    result
}
/// Authenticate the launched process completion while retaining cleanup ownership.
fn completion(
    probe: &Probe<'_>,
    listener: &UnixListener,
    child: &mut Launcher<'_>,
    deadline: Instant,
    clock: &dyn Clock,
) -> Result<(), Failure> {
    let pid =
        Pid::from_raw(child.child.id().try_into().map_err(|_| refused())?).ok_or_else(refused)?;
    let pidfd = pidfd_open(pid, PidfdFlags::empty()).map_err(|_| refused())?;
    let mut pending = [
        PollFd::new(listener, PollFlags::IN),
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
        || !child.child.wait().map_err(|_| refused())?.success()
    {
        return Err(refused());
    }
    // A second completion is never ignored, including one queued with child exit.
    let mut extra = [PollFd::new(listener, PollFlags::IN)];
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
    use super::{
        Clock, Duration, Instant, Launcher, Probe, UnixListener, UnixStream, authenticated_report,
        completion, timeout,
    };
    use maestro_kernel::retrieval::SystemClock;
    use maestro_test_scratch::disk_scratch_directory;
    use rustix::{
        event::{PollFd, PollFlags, Timespec, poll},
        io::Errno,
        process::{
            Pid, PidfdFlags, Signal, WaitOptions, geteuid, pidfd_open, pidfd_send_signal, waitpid,
        },
    };
    use std::{
        fs,
        io::Write as _,
        os::fd::OwnedFd,
        path::Path,
        process::{ChildStdin, Command, Stdio},
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

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

    /// At the post-poll clock observation, release a child whose exit was not ready.
    /// All frame bytes are queued before completion, so its read is one complete frame.
    #[derive(Debug)]
    struct ExitClock {
        at: Instant,
        calls: AtomicUsize,
        release: Mutex<Option<ChildStdin>>,
        pidfd: OwnedFd,
    }
    impl Clock for ExitClock {
        fn now(&self) -> Instant {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 7
                && let Some(mut input) = self.release.lock().unwrap().take()
            {
                writeln!(input, "exit").unwrap();
                ready(&self.pidfd);
            }
            self.at
        }
    }
    /// Wait for an actual process event, with a safety deadline but no elapsed assertion.
    fn ready(fd: &OwnedFd) {
        let mut events = [PollFd::new(fd, PollFlags::IN)];
        poll(
            &mut events,
            Some(&Timespec::try_from(Duration::from_secs(30)).unwrap()),
        )
        .unwrap();
        assert!(events[0].revents().contains(PollFlags::IN));
    }

    #[test]
    fn n37_completion_requires_both_exit_readiness_and_success() {
        for (exited, code, accepted) in [(false, 0, false), (true, 0, true), (true, 1, false)] {
            let root = disk_scratch_directory().unwrap();
            let endpoint = root.join("completion.sock");
            let listener = UnixListener::bind(&endpoint).unwrap();
            let mut peer = UnixStream::connect(&endpoint).unwrap();
            writeln!(
                peer,
                r#"{{"create_denied":true,"edit_denied":true,"delete_denied":true}}"#
            )
            .unwrap();
            let mut child = Command::new("sh")
                .args(["-c", &format!("read -r token; exit {code}")])
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();
            let pid = Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
            let pidfd = pidfd_open(pid, PidfdFlags::empty()).unwrap();
            let mut release = child.stdin.take();
            if exited {
                writeln!(release.as_mut().unwrap(), "exit").unwrap();
                ready(&pidfd);
                release = None;
            }
            let clock = ExitClock {
                at: Instant::now(),
                calls: AtomicUsize::new(0),
                release: Mutex::new(release),
                pidfd,
            };
            let mut launcher = Launcher {
                child,
                clock: &clock,
            };
            let probe = Probe {
                launcher: Path::new("unused"),
                binary: Path::new("unused"),
                store: &root,
                endpoint: endpoint.clone(),
                uid: geteuid().as_raw(),
            };
            // The listener is ready; the blocked child cannot exit until the post-poll
            // observation. A one-nanosecond poll budget therefore snapshots no exit.
            let result = completion(
                &probe,
                &listener,
                &mut launcher,
                clock.at + Duration::from_nanos(1),
                &clock,
            );
            launcher.stop().unwrap();
            drop(launcher);
            drop(listener);
            drop(peer);
            fs::remove_dir_all(root).unwrap();
            assert_eq!(result.is_ok(), accepted, "ready={exited}, exit={code}");
        }
    }

    #[test]
    fn n37_launcher_drop_kills_and_reaps_without_explicit_stop() {
        let child = Command::new("sleep").arg("60").spawn().unwrap();
        let pid = Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
        let pidfd = pidfd_open(pid, PidfdFlags::empty()).unwrap();
        drop(Launcher {
            child,
            clock: &SystemClock,
        });
        let observed = waitpid(Some(pid), WaitOptions::NOHANG);
        // Independently clean up even when the Drop mutant leaves the child alive.
        let _ = pidfd_send_signal(&pidfd, Signal::KILL);
        let _ = waitpid(Some(pid), WaitOptions::empty());
        assert_eq!(
            observed.err(),
            Some(Errno::CHILD),
            "owned child was not reaped by Drop"
        );
    }
    #[test]
    fn debt_probe_send_delivers_exact_denials() {
        use super::{Report, send};
        use std::io::{BufRead as _, BufReader};
        let (mut sender, reader) = UnixStream::pair().unwrap();
        reader
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let report = Report {
            create_denied: true,
            edit_denied: false,
            delete_denied: true,
        };
        send(&mut sender, &report).unwrap();
        drop(sender);
        let mut line = String::new();
        assert!(BufReader::new(reader).read_line(&mut line).unwrap() > 0);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&line).unwrap(),
            serde_json::json!({"create_denied":true,"edit_denied":false,"delete_denied":true})
        );
    }
}
