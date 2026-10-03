//! Only the explicitly delegated per-run subtree is writable by this supervisor.
use super::port::Refusal;
use crate::policy::limits::Limits;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Checked delegated root, with the supervisor in a separate manager leaf.
#[derive(Debug)]
pub(super) struct Delegation {
    /// Owned scope/service root, never a systemd-owned slice.
    root: PathBuf,
}
impl Delegation {
    /// Establish the no-internal-process layout before any worker starts.
    pub(super) fn new(root: PathBuf) -> Result<Self, Refusal> {
        let current = fs::read_to_string("/proc/self/cgroup").map_err(|_| Refusal::Unsupported)?;
        let current = current
            .strip_prefix("0::")
            .ok_or(Refusal::Unsupported)?
            .trim();
        if root != Path::new("/sys/fs/cgroup").join(current.trim_start_matches('/'))
            || !root.file_name().is_some_and(|name| {
                name.to_string_lossy().ends_with(".scope")
                    || name.to_string_lossy().ends_with(".service")
            })
        {
            return Err(Refusal::Configuration);
        }
        let controllers = fs::read_to_string(root.join("cgroup.controllers"))
            .map_err(|_| Refusal::Unsupported)?;
        for controller in ["cpu", "memory", "pids"] {
            if !controllers
                .split_whitespace()
                .any(|item| item == controller)
            {
                return Err(Refusal::Unsupported);
            }
        }
        let manager = root.join("manager");
        fs::create_dir(&manager).map_err(|_| Refusal::Unsupported)?;
        // The dedicated unit may also contain trusted Cargo/provisioner ancestors.
        // They must share the manager leaf, not violate no-internal-process.
        let managers =
            fs::read_to_string(root.join("cgroup.procs")).map_err(|_| Refusal::Unsupported)?;
        for pid in managers.lines() {
            fs::write(manager.join("cgroup.procs"), pid).map_err(|_| Refusal::Unsupported)?;
        }
        if !fs::read_to_string(root.join("cgroup.procs"))
            .map_err(|_| Refusal::Unsupported)?
            .trim()
            .is_empty()
        {
            return Err(Refusal::Unsupported);
        }
        fs::write(root.join("cgroup.subtree_control"), "+cpu +memory +pids")
            .map_err(|_| Refusal::Unsupported)?;
        Ok(Self { root })
    }
    /// Create an empty bounded leaf and verify every limit before releasing launch.
    pub(super) fn worker(&self, name: &str, limits: &Limits, pids: u64) -> Result<Worker, Refusal> {
        let path = self.root.join(name);
        fs::create_dir(&path).map_err(|_| Refusal::Containment)?;
        let worker = Worker { path };
        let memory = limits.memory_bytes.min(limits.decode.memory_bytes).get();
        let settings = plan(memory, limits.cpu_millicores.get(), pids);
        let result = (|| {
            for (file, value) in settings {
                setting(&worker.path, file, &value)?;
            }
            // Require the whole-tree kill mechanism before the parser exists.
            fs::OpenOptions::new()
                .write(true)
                .open(worker.path.join("cgroup.kill"))
                .map_err(|_| Refusal::Unsupported)?;
            Ok(())
        })();
        if let Err(error) = result {
            worker.remove()?;
            return Err(error);
        }
        Ok(worker)
    }
}
/// An owned worker leaf. All paths originate from the checked delegation.
#[derive(Debug)]
pub(super) struct Worker {
    /// Never exposed through a worker mount or inherited descriptor.
    path: PathBuf,
}
impl Worker {
    /// Attach the barrier-blocked bootstrap, not the trusted supervisor.
    pub(super) fn attach(&self, pid: u32) -> Result<(), Refusal> {
        fs::write(self.path.join("cgroup.procs"), pid.to_string()).map_err(|_| Refusal::Containment)
    }
    /// Kill every descendant, including reparented/forked workers.
    pub(super) fn kill(&self) -> Result<(), Refusal> {
        fs::write(self.path.join("cgroup.kill"), "1").map_err(|_| Refusal::Cleanup)
    }
    /// Kernel proof that no descendant remains, not only a leader's wait status.
    pub(super) fn empty(&self) -> Result<bool, Refusal> {
        let events =
            fs::read_to_string(self.path.join("cgroup.events")).map_err(|_| Refusal::Cleanup)?;
        Ok(events.lines().any(|line| line == "populated 0"))
    }
    /// Actual kernel OOM evidence, not an allocation failure guessed from exit.
    pub(super) fn oom_killed(&self) -> Result<bool, Refusal> {
        let events =
            fs::read_to_string(self.path.join("memory.events")).map_err(|_| Refusal::Cleanup)?;
        Ok(events.lines().any(|line| {
            line.strip_prefix("oom_kill ")
                .is_some_and(|value| value != "0")
        }))
    }
    /// Remove only this owned empty leaf; failures suppress all output.
    pub(super) fn remove(self) -> Result<(), Refusal> {
        if !self.empty()? {
            return Err(Refusal::Cleanup);
        }
        fs::remove_dir(&self.path).map_err(|_| Refusal::Cleanup)
    }
}

/// Write/read-back one requested kernel controller setting.
fn setting(root: &Path, name: &str, value: &str) -> Result<(), Refusal> {
    let path = root.join(name);
    fs::write(&path, value).map_err(|_| Refusal::Unsupported)?;
    if fs::read_to_string(path)
        .map_err(|_| Refusal::Unsupported)?
        .trim()
        != value
    {
        return Err(Refusal::Unsupported);
    }
    Ok(())
}

/// Controller plan consumes composed envelopes, with no production default values.
fn plan(memory: u64, cpu: u64, pids: u64) -> [(&'static str, String); 5] {
    let quota = u128::from(cpu) * 100;
    [
        ("memory.max", memory.to_string()),
        ("memory.swap.max", "0".into()),
        ("memory.oom.group", "1".into()),
        ("pids.max", pids.to_string()),
        ("cpu.max", format!("{quota} 100000")),
    ]
}
#[cfg(test)]
mod tests {
    use super::plan;
    #[test]
    fn n17_default_controller_plan_preserves_units_and_no_swap() {
        assert_eq!(
            plan(123, 2000, 16),
            [
                ("memory.max", "123".into()),
                ("memory.swap.max", "0".into()),
                ("memory.oom.group", "1".into()),
                ("pids.max", "16".into()),
                ("cpu.max", "200000 100000".into()),
            ]
        );
        assert_eq!(
            plan(1, u64::MAX, 2).last().unwrap().1,
            "1844674407370955161500 100000"
        );
    }
}
