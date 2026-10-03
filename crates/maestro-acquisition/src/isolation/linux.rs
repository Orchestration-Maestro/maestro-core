//! Qualified Linux adapter. Host configuration never auto-detects a weaker posture.
use super::{
    cgroup::Delegation,
    launch::{self, Configuration},
    port::{Isolation, Launch, PinnedFile, Refusal},
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
    sync::Mutex,
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
    host: Host,
    /// Serialize scratch recovery and owned launches in this adapter.
    // ponytail: one run per adapter; separate scoped adapters for parallel work.
    owned: Mutex<()>,
}
impl Linux {
    /// Establish the trusted manager layout; absence of any controller refuses.
    /// # Errors
    /// Wrong unit ownership, unsupported delegation or insecure scratch parent.
    pub fn new(host: Host) -> Result<Self, Refusal> {
        let metadata = fs::symlink_metadata(&host.scratch).map_err(|_| Refusal::Configuration)?;
        if !metadata.is_dir()
            || metadata.permissions().mode() & 0o777 != 0o700
            || !host.scratch.is_absolute()
            || host.apparmor_required && host.bootstrap_mode != BootstrapMode::Installed
        {
            return Err(Refusal::Configuration);
        }
        let delegation = Delegation::new(host.delegated.clone())?;
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
        scratch::recover(&self.host.scratch)?;
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
