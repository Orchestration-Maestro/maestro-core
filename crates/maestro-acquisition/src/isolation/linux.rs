//! Qualified Linux adapter. Host configuration never auto-detects a weaker posture.
use super::{
    cgroup::Delegation,
    cgroup_host::HostIo,
    launch::{self, Configuration},
    port::{CgroupIo, Isolation, Launch, PinnedFile, Refusal},
    scratch::{self, Preparing},
    supervision::{self, Timing},
};
use crate::{
    extraction::decode::{DecodeRefusal, DecodeStage, ParserDecode},
    transport::stream::Accounting,
};
use maestro_kernel::acquisition::Handle;
use rustix::fs::{OFlags, fcntl_setfl};
use std::{
    fs,
    io::Write as _,
    os::{fd::AsRawFd as _, unix::fs::PermissionsExt as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

/// Explicit approved host posture, never a permissive fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapMode {
    /// WSL/no `AppArmor` userns restriction: digest-checked sealed memfd copy.
    Sealed,
    /// Ubuntu: verified administrator-owned inode preserves `AppArmor` attachment.
    Installed,
}
impl BootstrapMode {
    /// Parse explicit configuration. Pi has no equivalent sandbox posture.
    /// # Errors
    /// Missing or unknown mode refuses before launch.
    pub fn parse(value: Option<&str>) -> Result<Self, Refusal> {
        match value {
            Some("sealed") => Ok(Self::Sealed),
            Some("installed") => Ok(Self::Installed),
            _ => Err(Refusal::Configuration),
        }
    }
}
/// Scoped unit and scratch supplied by the trusted host provisioner.
#[derive(Debug)]
pub struct Host {
    /// Dedicated per-run systemd scope/service cgroup root.
    pub delegated: PathBuf,
    /// Dedicated 0700 scratch parent; worker sees only its `root` subdirectory.
    pub scratch: PathBuf,
    /// Provisioned bootstrap's read-only handle and exact digest.
    pub bootstrap: PinnedFile,
    /// Required explicit posture, with no default/auto-detection.
    pub bootstrap_mode: BootstrapMode,
    /// Require launcher-only `AppArmor` profile attachment when CI provisioned it.
    pub apparmor_required: bool,
}
/// Linux namespace/Landlock/seccomp/cgroup adapter; no plain-process fallback.
#[derive(Debug)]
pub struct Linux {
    /// Checked controller delegation, shared without exposing it to workers.
    delegation: Delegation,
    /// Trusted host configuration.
    pub(super) host: Host,
    /// Serialize scratch recovery and owned launches in this adapter.
    // ponytail: one run per adapter; separate scoped adapters for parallel work.
    owned: Mutex<()>,
}
impl Linux {
    /// Establish the trusted manager layout; absence of any controller refuses.
    /// # Errors
    /// Wrong unit ownership, unsupported delegation or insecure scratch parent.
    pub fn new(host: Host) -> Result<Self, Refusal> {
        Self::with_io(host, Arc::new(HostIo))
    }
    /// Private construction keeps both effect implementations on the same driver.
    pub(super) fn with_io(host: Host, io: Arc<dyn CgroupIo>) -> Result<Self, Refusal> {
        let metadata = fs::symlink_metadata(&host.scratch).map_err(|_| Refusal::Configuration)?;
        if !metadata.is_dir()
            || metadata.permissions().mode() & 0o777 != 0o700
            || !host.scratch.is_absolute()
            || host.apparmor_required && host.bootstrap_mode != BootstrapMode::Installed
        {
            return Err(Refusal::Configuration);
        }
        let delegation = Delegation::with_io(host.delegated.clone(), io)?;
        Ok(Self {
            delegation,
            host,
            owned: Mutex::new(()),
        })
    }
    /// Build snapshots before the bootstrap is attached/released in its bounded leaf.
    fn execute(
        &self,
        run: &Path,
        name: &str,
        mut request: Launch,
        accounting: &mut Accounting,
    ) -> Result<Vec<u8>, Refusal> {
        let limits = accounting.limits().clone();
        let mut remaining = limits.staging_bytes.get();
        let bootstrap = launch::bootstrap(
            PinnedFile {
                file: self
                    .host
                    .bootstrap
                    .file
                    .try_clone()
                    .map_err(|_| Refusal::LaunchPin)?,
                digest: self.host.bootstrap.digest.clone(),
            },
            self.host.bootstrap_mode == BootstrapMode::Installed,
            &mut remaining,
        )?;
        let root = run.join("root");
        fs::create_dir(&root).map_err(|_| Refusal::Containment)?;
        let (parser, interpreter) = launch::prepare(&root, &mut request, &mut remaining)?;
        let (clock, deadline) = accounting
            .read_timing()
            .map_err(|_| Refusal::Configuration)?;
        let config = Configuration {
            root,
            parser_fd: parser.as_raw_fd(),
            bootstrap_fd: bootstrap.as_raw_fd(),
            arguments: request.arguments,
            interpreter,
            memory_bytes: limits.memory_bytes.min(limits.decode.memory_bytes).get(),
        };
        let encoded = serde_json::to_string(&config).map_err(|_| Refusal::Configuration)?;
        if encoded.len() > 4 * 1024 * 1024 {
            return Err(Refusal::Configuration);
        }
        let worker = self.delegation.worker(name, &limits, request.pids_max)?;
        let mut command = Command::new(launch::executable(&bootstrap));
        command
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if self.host.apparmor_required {
            command.env("MAESTRO_N17_PROFILE", "required");
        }
        let spawned = command.spawn();
        let Ok(mut child) = spawned else {
            worker.remove()?;
            return Err(Refusal::Containment);
        };
        let timing = Timing {
            clock,
            deadline,
            cancelled: request.cancelled,
        };
        let mut decode = ParserDecode::new(accounting);
        let result = (|| {
            worker.attach(child.id())?;
            let stdin = child.stdin.as_mut().ok_or(Refusal::Containment)?;
            writeln!(stdin, "{encoded}").map_err(|_| Refusal::Containment)?;
            fcntl_setfl(&*stdin, OFlags::NONBLOCK).map_err(|_| Refusal::Containment)?;
            supervision::run(
                &mut child,
                &mut decode,
                &timing,
                limits
                    .memory_bytes
                    .min(limits.decode.memory_bytes)
                    .min(limits.staging_bytes)
                    .get(),
            )
        })();
        // Also kill an unattached, barrier-blocked bootstrap if attach itself failed.
        if result.is_err() {
            decode.crashed(DecodeStage::Attachment);
        }
        let cleanup = supervision::cleanup(&worker, &mut child);
        let memory = worker.oom_killed();
        let removed = worker.remove();
        cleanup?;
        removed?;
        if memory? {
            return Err(Refusal::Memory);
        }
        result
    }
}
impl Isolation for Linux {
    fn run(&self, request: Launch, accounting: &mut Accounting) -> Result<Vec<u8>, Refusal> {
        let _owned = self.owned.lock().map_err(|_| Refusal::Cleanup)?;
        scratch::recover_with(&self.host.scratch, &|path| self.delegation.io.read(path))?;
        accounting.deadline().map_err(|reason| {
            Refusal::Decode(DecodeRefusal {
                stage: None,
                reason,
            })
        })?;
        let name = format!("n17-{}", Handle::new());
        let run = self.host.scratch.join(&name);
        Preparing::new(&self.host.scratch, &name, &self.host.delegated.join(&name))?
            .commit(&run)?;
        let result = self.execute(&run, &name, request, accounting);
        // Preserve the ownership receipt if teardown is unproven. Recovery then
        // refuses an active tree instead of losing evidence and launching anew.
        if result == Err(Refusal::Cleanup) {
            return result;
        }
        fs::remove_dir_all(run).map_err(|_| Refusal::Cleanup)?;
        result
    }
}

#[cfg(test)]
mod driver_tests {
    use super::{BootstrapMode, Linux};
    use crate::isolation::{
        port::{Isolation as _, Refusal},
        test_support::{DONE, Groups, accounting, directory, driver, pin, request},
    };
    use crate::{isolation::test_support::FixedClock, transport::stream::Accounting};
    use std::{fs, path::Path, sync::Arc};
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        str::from_utf8,
        sync::{Mutex, atomic::Ordering},
        time::{Duration, Instant},
    };

    #[test]
    fn n17_linux_real_spawn_attach_barrier_min_envelope_and_lifecycle() {
        let parent = directory();
        let io = Groups::new();
        let release = parent.join("attached");
        *io.barrier_release.lock().unwrap() = Some(release.clone());
        let script = format!(
            "#!/bin/sh\nwhile test ! -f '{}' ; do :; done\n{}",
            release.display(),
            from_utf8(DONE)
                .unwrap()
                .strip_prefix("#!/bin/sh\n")
                .unwrap(),
        );
        let driver = driver(&parent, &io, script.as_bytes());
        io.calls.lock().unwrap().clear();
        let mut ledger = accounting();
        let mut bounds = ledger.limits().clone();
        bounds.memory_bytes = 800_000.try_into().unwrap();
        bounds.decode.memory_bytes = 500_000.try_into().unwrap();
        bounds.staging_bytes = 600_000.try_into().unwrap();
        ledger.tighten(&bounds);
        assert_eq!(driver.run(request(&parent), &mut ledger), Ok(vec![]));
        assert_eq!(fs::read_dir(parent.join("runs")).unwrap().count(), 0);
        let calls = io.calls.lock().unwrap();
        assert!(
            calls
                .iter()
                .any(|call| call.ends_with("/memory.max 500000"))
        );
        let attach = calls
            .iter()
            .position(|call| call.contains("/cgroup.procs "))
            .unwrap();
        let kill = calls
            .iter()
            .position(|call| call.ends_with("/cgroup.kill 1"))
            .unwrap();
        assert!(attach < kill);
        assert_eq!(
            calls.last().unwrap().split_whitespace().next(),
            Some("remove")
        );
        drop(calls);
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn n17_linux_spawn_attach_cleanup_oom_and_decode_crash_priority() {
        for (index, failure, expected, preserve) in [
            (0, Some(1), Refusal::Containment, false),
            (1, Some(13), Refusal::Containment, false),
            (2, Some(14), Refusal::Cleanup, true),
            (3, Some(16), Refusal::Cleanup, true),
            (4, Some(18), Refusal::Cleanup, true),
        ] {
            let parent = directory();
            let io = Groups::new();
            let driver = driver(&parent, &io, DONE);
            io.calls.lock().unwrap().clear();
            *io.fail.lock().unwrap() = failure;
            let mut ledger = accounting();
            assert_eq!(
                driver.run(request(&parent), &mut ledger),
                Err(expected),
                "{index}"
            );
            assert_eq!(
                fs::read_dir(parent.join("runs")).unwrap().count(),
                usize::from(preserve)
            );
            if failure == Some(13) {
                assert!(
                    crate::extraction::decode::ParserDecode::new(&mut ledger)
                        .finish()
                        .is_err()
                );
            }
            fs::remove_dir_all(parent).unwrap();
        }
        for (script, expected) in [
            (b"not executable".as_slice(), Refusal::Containment),
            (
                b"#!/bin/sh\nread -r config\nprintf '%s\\n' '{\"kind\":\"ready\"}'; exit 7\n",
                Refusal::Crash,
            ),
        ] {
            let parent = directory();
            let io = Groups::new();
            let driver = driver(&parent, &io, script);
            assert_eq!(
                driver.run(request(&parent), &mut accounting()),
                Err(expected)
            );
            assert_eq!(fs::read_dir(parent.join("runs")).unwrap().count(), 0);
            fs::remove_dir_all(parent).unwrap();
        }
        let parent = directory();
        let io = Groups::new();
        let driver = driver(&parent, &io, DONE);
        *io.oom.lock().unwrap() = "oom_kill 1".into();
        assert_eq!(
            driver.run(request(&parent), &mut accounting()),
            Err(Refusal::Memory)
        );
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn n17_linux_lock_recovery_deadline_order_and_receipt_preservation() {
        let parent = directory();
        let io = Groups::new();
        let driver = driver(&parent, &io, DONE);
        let guard = driver.owned.lock().unwrap();
        drop(guard);
        let foreign = parent.join("runs/foreign");
        fs::create_dir(&foreign).unwrap();
        let mut ledger = accounting();
        assert_eq!(
            driver.run(request(&parent), &mut ledger),
            Err(Refusal::Cleanup)
        );
        assert!(foreign.is_dir());
        fs::remove_dir(&foreign).unwrap();
        let now = Instant::now();
        let clock = Arc::new(FixedClock(Mutex::new(now)));
        let mut ledger = Accounting::with_clock(accounting().limits().clone(), clock.clone());
        *clock.0.lock().unwrap() = now + Duration::from_secs(5);
        assert!(matches!(
            driver.run(request(&parent), &mut ledger),
            Err(Refusal::Decode(_))
        ));
        assert_eq!(fs::read_dir(parent.join("runs")).unwrap().count(), 0);
        let _poison = catch_unwind(AssertUnwindSafe(|| {
            let _guard = driver.owned.lock().unwrap();
            panic!("poison synthetic owner lock");
        }));
        assert_eq!(
            driver.run(request(&parent), &mut accounting()),
            Err(Refusal::Cleanup)
        );
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn n17_linux_closed_barrier_write_failure_and_installed_posture() {
        let parent = directory();
        let io = Groups::new();
        let adapter = driver(
            &parent,
            &io,
            b"#!/bin/sh\nexec 0<&-\nprintf '%s\\n' '{\"kind\":\"ready\"}' '{\"kind\":\"done\"}'\n",
        );
        io.closed_barrier.store(true, Ordering::Release);
        let mut ledger = accounting();
        assert_eq!(
            adapter.run(request(&parent), &mut ledger),
            Err(Refusal::Containment)
        );
        assert!(
            crate::extraction::decode::ParserDecode::new(&mut ledger)
                .finish()
                .is_err()
        );
        assert_eq!(fs::read_dir(parent.join("runs")).unwrap().count(), 0);
        fs::remove_dir_all(parent).unwrap();
        let parent = directory();
        let io = Groups::new();
        let adapter = driver(&parent, &io, DONE);
        let mut host = adapter.host;
        host.bootstrap = pin(Path::new("/bin/true"));
        host.bootstrap_mode = BootstrapMode::Installed;
        host.apparmor_required = true;
        let adapter = Linux::with_io(host, io).unwrap();
        // This installed ordinary image has no launch barrier: it may exit before
        // the parent's write or at EOF. Both outcomes must hold, never release.
        assert!(matches!(
            adapter.run(request(&parent), &mut accounting()),
            Err(Refusal::Unsupported | Refusal::Containment)
        ));
        assert_eq!(fs::read_dir(parent.join("runs")).unwrap().count(), 0);
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn n17_linux_cancellation_uses_anchored_clock_after_prior_stages() {
        let parent = directory();
        let io = Groups::new();
        let adapter = driver(&parent, &io, DONE);
        let now = Instant::now();
        let clock = Arc::new(FixedClock(Mutex::new(now)));
        let mut ledger = Accounting::with_clock(accounting().limits().clone(), clock.clone());
        ledger.deadline().unwrap();
        *clock.0.lock().unwrap() = now + Duration::from_secs(4);
        let original = ledger.read_timing().unwrap().1;
        let launch = request(&parent);
        launch.cancelled.store(true, Ordering::Release);
        assert_eq!(adapter.run(launch, &mut ledger), Err(Refusal::Cancelled));
        assert_eq!(ledger.read_timing().unwrap().1, original);
        fs::remove_dir_all(parent).unwrap();
    }
}
