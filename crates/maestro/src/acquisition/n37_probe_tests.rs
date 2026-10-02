//! The real qualifier launcher must reap a hostile long-running owned probe.
use super::authority_probe::{Probe, qualify};
use maestro_acquisition::lifecycle::resume::stop_owned;
use maestro_kernel::retrieval::Clock;
use maestro_test_scratch::disk_scratch_directory;
use rustix::process::{
    Pid, PidfdFlags, Signal, WaitOptions, geteuid, pidfd_open, pidfd_send_signal, waitpid,
};
use std::{
    env, fs,
    io::Write as _,
    os::unix::{fs::PermissionsExt as _, net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, id},
    thread,
    time::{Duration, Instant},
};

/// Keep the unrelated fixture process owned even when an assertion fails.
struct ForeignChild(Child);
impl Drop for ForeignChild {
    fn drop(&mut self) {
        drop(self.0.kill());
        drop(self.0.wait());
    }
}

/// Mutation failures must not leak the launched probe; signal only our recorded child.
struct ProbeCleanup(PathBuf);
impl Drop for ProbeCleanup {
    fn drop(&mut self) {
        let Some(pid) = fs::read_to_string(&self.0)
            .ok()
            .and_then(|text| text.parse::<i32>().ok())
            .and_then(Pid::from_raw)
        else {
            return;
        };
        let Ok(fd) = pidfd_open(pid, PidfdFlags::empty()) else {
            return;
        };
        let Ok(status) = fs::read_to_string(format!("/proc/{}/status", pid.as_raw_pid())) else {
            return;
        };
        let parent = format!("PPid:\t{}", id());
        if status.lines().any(|line| line == parent) {
            let _ = pidfd_send_signal(&fd, Signal::KILL);
            let _ = waitpid(Some(pid), WaitOptions::empty());
        }
    }
}

/// Deadlines bound safety; no test asserts elapsed wall time.
#[derive(Debug)]
struct HostClock;
impl Clock for HostClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[test]
fn n37_long_running_probe_child() {
    let Ok(endpoint) = env::var("MAESTRO_N37_PROBE_ENDPOINT") else {
        return;
    };
    let pid = env::var("MAESTRO_N37_PROBE_PID").unwrap();
    fs::write(pid, id().to_string()).unwrap();
    let mut stream = UnixStream::connect(endpoint).unwrap();
    writeln!(
        stream,
        r#"{{"create_denied":true,"edit_denied":false,"delete_denied":true}}"#
    )
    .unwrap();
    loop {
        thread::park();
    }
}

#[test]
fn n37_real_probe_launcher_cancels_and_reaps_only_its_owned_process() {
    let root = disk_scratch_directory().unwrap();
    let launcher = root.join("launcher");
    fs::write(
        &launcher,
        b"#!/bin/sh\n\
export MAESTRO_N37_PROBE_PID=\"$6\"\n\
export MAESTRO_N37_PROBE_ENDPOINT=\"$8\"\n\
exec \"$2\" --exact acquisition::n37_probe_tests::n37_long_running_probe_child --nocapture\n",
    )
    .unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
    let executable = env::current_exe().unwrap();
    let pid_file = root.join("owned.pid");
    let _cleanup = ProbeCleanup(pid_file.clone());
    let mut foreign = ForeignChild(Command::new("sleep").arg("60").spawn().unwrap());
    let probe = Probe {
        launcher: &launcher,
        binary: &executable,
        store: &pid_file,
        endpoint: root.join("probe.sock"),
        uid: geteuid().as_raw(),
    };
    assert!(
        qualify(&probe, Duration::from_secs(30), &HostClock).is_err(),
        "hostile probe accepted"
    );
    let pid = fs::read_to_string(&pid_file).unwrap();
    assert!(
        !Path::new("/proc").join(pid.trim()).exists(),
        "owned launcher not reaped"
    );
    assert!(!probe.endpoint.exists(), "owned socket leaked");
    assert!(
        foreign.0.try_wait().unwrap().is_none(),
        "foreign process killed"
    );
    stop_owned(
        &mut foreign.0,
        Instant::now() + Duration::from_secs(30),
        &HostClock,
    )
    .unwrap();
    fs::remove_dir_all(root).unwrap();
}
