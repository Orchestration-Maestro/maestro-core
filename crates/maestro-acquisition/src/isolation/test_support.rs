//! Synthetic default-feature envelopes and ordered group effects, not kernel proof.
#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "authored synthetic fixture helpers; panics fail the calling test"
)]
use super::{
    linux::{BootstrapMode, Host, Linux},
    port::{CgroupIo, Launch, PinnedFile},
};
use crate::{policy::limits::Limits, transport::stream::Accounting};
use maestro_kernel::{artifact::Digest, retrieval::Clock};
use maestro_test_scratch::scratch_directory;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File},
    io,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// Explicit synthetic fixture bounds, never production defaults.
pub(super) fn limits() -> Limits {
    let policy: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/policy.json")).unwrap();
    let mut limits: Limits =
        serde_json::from_value(policy["sources"][0]["limits"].clone()).unwrap();
    limits.wire_bytes = 1000.try_into().unwrap();
    limits.elapsed_ms = 5000.try_into().unwrap();
    limits.decode.elapsed_ms = limits.elapsed_ms;
    limits.staging_bytes = 10_000_000.try_into().unwrap();
    limits.memory_bytes = 1_000_000.try_into().unwrap();
    limits.decode.memory_bytes = limits.memory_bytes;
    limits.decode.expanded_bytes = 100.try_into().unwrap();
    limits.decode.expansion_ratio = 10.try_into().unwrap();
    limits.decode.nested_levels = 10.try_into().unwrap();
    limits.decode.members = 10.try_into().unwrap();
    limits
}
/// Ledger includes prior encoded-stage costs.
pub(super) fn accounting() -> Accounting {
    let mut ledger = Accounting::new(limits());
    ledger.encoded(100).unwrap();
    ledger.deadline().unwrap();
    ledger
}
/// Manually anchored clock for exact boundary tests.
#[derive(Debug)]
pub(super) struct FixedClock(pub(super) Mutex<Instant>);
impl Clock for FixedClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}
/// Fresh private directory with deterministic cleanup by its owner.
pub(super) fn directory() -> PathBuf {
    let path = scratch_directory().unwrap();
    fs::create_dir_all(&path).unwrap();
    path
}
/// Read-only synthetic input pin.
pub(super) fn pin(path: &Path) -> PinnedFile {
    PinnedFile {
        file: File::open(path).unwrap(),
        digest: Digest::of(&fs::read(path).unwrap()),
    }
}
/// Valid synthetic ELF metadata, deliberately not executable code.
pub(super) fn image() -> Vec<u8> {
    let mut bytes = vec![0; 120];
    for (offset, field) in [
        (0, b"\x7fELF\x02\x01\x01".as_slice()),
        (18, &62_u16.to_le_bytes()),
        (32, &64_u64.to_le_bytes()),
        (54, &56_u16.to_le_bytes()),
        (56, &1_u16.to_le_bytes()),
        (64, &0x6474_e551_u32.to_le_bytes()),
        (68, &6_u32.to_le_bytes()),
    ] {
        bytes
            .get_mut(offset..offset + field.len())
            .unwrap()
            .copy_from_slice(field);
    }
    bytes
}
/// Scoped native request; callers own the scratch holding its read handles.
pub(super) fn request(parent: &Path) -> Launch {
    let path = parent.join("image");
    fs::write(&path, image()).unwrap();
    Launch {
        parser: pin(&path),
        runtime: vec![],
        inputs: vec![],
        arguments: vec![],
        pids_max: 2,
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}
/// Observable I/O sequence with one selected failing effect.
#[derive(Debug, Default)]
pub(super) struct Groups {
    pub(super) calls: Mutex<Vec<String>>,
    pub(super) files: Mutex<HashMap<PathBuf, String>>,
    pub(super) fail: Mutex<Option<usize>>,
    pub(super) events: Mutex<VecDeque<String>>,
    pub(super) oom: Mutex<String>,
    pub(super) observations: Mutex<HashMap<PathBuf, String>>,
    pub(super) closed_barrier: AtomicBool,
    pub(super) barrier_release: Mutex<Option<PathBuf>>,
}
impl Groups {
    /// Checked synthetic service with trusted manager ancestors.
    pub(super) fn new() -> Arc<Self> {
        let io = Arc::new(Self {
            oom: Mutex::new("oom_kill 0".into()),
            ..Self::default()
        });
        let mut files = io.files.lock().unwrap();
        for (path, text) in [
            ("/proc/self/cgroup", "0::/n17.service\n"),
            (
                "/sys/fs/cgroup/n17.service/cgroup.controllers",
                "cpu memory pids",
            ),
            ("/sys/fs/cgroup/n17.service/cgroup.procs", "12\n34\n"),
        ] {
            files.insert(PathBuf::from(path), text.into());
        }
        drop(files);
        io
    }
    /// The child waits on a separate file handshake, so barrier bytes cannot disappear.
    fn before_release(&self, pid: &str) -> io::Result<()> {
        let release = self.barrier_release.lock().unwrap();
        let Some(path) = release.as_ref() else {
            return Ok(());
        };
        let input = File::open(format!("/proc/{pid}/fd/0"))?;
        let mut pending = [PollFd::new(&input, PollFlags::IN)];
        poll(
            &mut pending,
            Some(&Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }),
        )?;
        fs::write(path, b"attached")?;
        if !pending.first().unwrap().revents().is_empty() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(())
    }
    /// Record before failure so tests see attempted effects too.
    fn effect(&self, text: String) -> io::Result<()> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(text);
        if *self.fail.lock().unwrap() == Some(calls.len()) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }
}
impl CgroupIo for Groups {
    fn read(&self, path: &Path) -> io::Result<String> {
        self.effect(format!("read {}", path.display()))?;
        if let Some(value) = self.observations.lock().unwrap().get(path) {
            return Ok(value.clone());
        }
        match path.file_name().and_then(|n| n.to_str()) {
            Some("cgroup.events") => Ok(self
                .events
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| "populated 0".into())),
            Some("memory.events") => Ok(self
                .files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .unwrap_or_else(|| self.oom.lock().unwrap().clone())),
            _ => self
                .files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| io::ErrorKind::NotFound.into()),
        }
    }
    fn write(&self, path: &Path, value: &str) -> io::Result<()> {
        self.effect(format!("write {} {value}", path.display()))?;
        if path.ends_with("cgroup.procs") && !path.ends_with("manager/cgroup.procs") {
            self.before_release(value)?;
        }
        if self.closed_barrier.load(Ordering::Acquire)
            && path.ends_with("cgroup.procs")
            && !path.ends_with("manager/cgroup.procs")
        {
            wait_closed(value)?;
        }
        self.files
            .lock()
            .unwrap()
            .insert(path.to_owned(), value.into());
        if path.ends_with("manager/cgroup.procs") {
            self.files.lock().unwrap().insert(
                path.parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("cgroup.procs"),
                String::new(),
            );
        }
        Ok(())
    }
    fn create(&self, path: &Path) -> io::Result<()> {
        self.effect(format!("create {}", path.display()))
    }
    fn remove(&self, path: &Path) -> io::Result<()> {
        self.effect(format!("remove {}", path.display()))
    }
    fn probe_kill(&self, path: &Path) -> io::Result<()> {
        self.effect(format!("probe {}", path.display()))
    }
}

/// Observe the disposable child closing its read end before the driver writes.
fn wait_closed(pid: &str) -> io::Result<()> {
    let descriptor = PathBuf::from(format!("/proc/{pid}/fd/0"));
    let until = Instant::now() + Duration::from_secs(5);
    while descriptor.exists() {
        if Instant::now() >= until {
            return Err(io::ErrorKind::TimedOut.into());
        }
        thread::yield_now();
    }
    Ok(())
}

/// Unprivileged shell image only in the private test driver, never a public fallback.
pub(super) fn driver(parent: &Path, io: &Arc<Groups>, script: &[u8]) -> Linux {
    let scratch = parent.join("runs");
    fs::create_dir(&scratch).unwrap();
    fs::set_permissions(&scratch, fs::Permissions::from_mode(0o700)).unwrap();
    let bootstrap = parent.join("bootstrap");
    fs::write(&bootstrap, script).unwrap();
    Linux::with_io(
        Host {
            delegated: PathBuf::from("/sys/fs/cgroup/n17.service"),
            scratch,
            bootstrap: pin(&bootstrap),
            bootstrap_mode: BootstrapMode::Sealed,
            apparmor_required: false,
        },
        io.clone(),
    )
    .unwrap()
}
pub(super) const DONE: &[u8] = concat!(
    "#!/bin/sh\nread -r config || exit 8\n",
    "test -z \"$HOME$HTTPS_PROXY\" || exit 9\n",
    "printf '%s\\n' '{\"kind\":\"ready\"}' '{\"kind\":\"done\"}'\n",
)
.as_bytes();
