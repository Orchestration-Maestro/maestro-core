//! Only the explicitly delegated per-run subtree is writable by this supervisor.
use super::port::{CgroupIo, Refusal};
use crate::policy::limits::Limits;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Checked delegated root, with the supervisor in a separate manager leaf.
#[derive(Debug)]
pub(super) struct Delegation {
    /// Owned scope/service root, never a systemd-owned slice.
    root: PathBuf,
    /// Mandatory real adapter in production; scripted effects in default tests.
    pub(super) io: Arc<dyn CgroupIo>,
}
impl Delegation {
    /// Establish the same checked layout through the private effect boundary.
    pub(super) fn with_io(root: PathBuf, io: Arc<dyn CgroupIo>) -> Result<Self, Refusal> {
        let current = io
            .read(Path::new("/proc/self/cgroup"))
            .map_err(|_| Refusal::Unsupported)?;
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
        let controllers = io
            .read(&root.join("cgroup.controllers"))
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
        io.create(&manager).map_err(|_| Refusal::Unsupported)?;
        // The dedicated unit may also contain trusted Cargo/provisioner ancestors.
        // They must share the manager leaf, not violate no-internal-process.
        let managers = io
            .read(&root.join("cgroup.procs"))
            .map_err(|_| Refusal::Unsupported)?;
        for pid in managers.lines() {
            io.write(&manager.join("cgroup.procs"), pid)
                .map_err(|_| Refusal::Unsupported)?;
        }
        if !io
            .read(&root.join("cgroup.procs"))
            .map_err(|_| Refusal::Unsupported)?
            .trim()
            .is_empty()
        {
            return Err(Refusal::Unsupported);
        }
        io.write(&root.join("cgroup.subtree_control"), "+cpu +memory +pids")
            .map_err(|_| Refusal::Unsupported)?;
        Ok(Self { root, io })
    }
    /// Create an empty bounded leaf and verify every limit before releasing launch.
    pub(super) fn worker(&self, name: &str, limits: &Limits, pids: u64) -> Result<Worker, Refusal> {
        let path = self.root.join(name);
        self.io.create(&path).map_err(|_| Refusal::Containment)?;
        let worker = Worker {
            path,
            io: self.io.clone(),
        };
        let memory = limits.memory_bytes.min(limits.decode.memory_bytes).get();
        let settings = plan(memory, limits.cpu_millicores.get(), pids);
        let result = (|| {
            for (file, value) in settings {
                setting(&*self.io, &worker.path, file, &value)?;
            }
            // Require the whole-tree kill mechanism before the parser exists.
            self.io
                .probe_kill(&worker.path.join("cgroup.kill"))
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
    /// Effects belong to the checked delegation.
    io: Arc<dyn CgroupIo>,
}
impl Worker {
    /// Attach the barrier-blocked bootstrap, not the trusted supervisor.
    pub(super) fn attach(&self, pid: u32) -> Result<(), Refusal> {
        self.io
            .write(&self.path.join("cgroup.procs"), &pid.to_string())
            .map_err(|_| Refusal::Containment)
    }
    /// Kill every descendant, including reparented/forked workers.
    pub(super) fn kill(&self) -> Result<(), Refusal> {
        self.io
            .write(&self.path.join("cgroup.kill"), "1")
            .map_err(|_| Refusal::Cleanup)
    }
    /// Kernel proof that no descendant remains, not only a leader's wait status.
    pub(super) fn empty(&self) -> Result<bool, Refusal> {
        let events = self
            .io
            .read(&self.path.join("cgroup.events"))
            .map_err(|_| Refusal::Cleanup)?;
        Ok(events.lines().any(|line| line == "populated 0"))
    }
    /// Actual kernel OOM evidence, not an allocation failure guessed from exit.
    pub(super) fn oom_killed(&self) -> Result<bool, Refusal> {
        let events = self
            .io
            .read(&self.path.join("memory.events"))
            .map_err(|_| Refusal::Cleanup)?;
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
        self.io.remove(&self.path).map_err(|_| Refusal::Cleanup)
    }
}

/// Write/read-back one requested kernel controller setting.
fn setting(io: &dyn CgroupIo, root: &Path, name: &str, value: &str) -> Result<(), Refusal> {
    let path = root.join(name);
    io.write(&path, value).map_err(|_| Refusal::Unsupported)?;
    if io.read(&path).map_err(|_| Refusal::Unsupported)?.trim() != value {
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

#[cfg(test)]
mod lifecycle_tests {
    use super::{Delegation, Refusal};
    use crate::isolation::test_support::{Groups, limits};
    use std::{path::PathBuf, sync::Arc};

    fn root() -> PathBuf {
        PathBuf::from("/sys/fs/cgroup/n17.service")
    }
    fn delegation(io: &Arc<Groups>) -> Delegation {
        Delegation::with_io(root(), io.clone()).unwrap()
    }
    #[test]
    fn n17_group_manager_migration_limit_readbacks_attach_and_teardown_order() {
        let io = Groups::new();
        let group = delegation(&io);
        assert_eq!(
            *io.calls.lock().unwrap(),
            vec![
                "read /proc/self/cgroup",
                "read /sys/fs/cgroup/n17.service/cgroup.controllers",
                "create /sys/fs/cgroup/n17.service/manager",
                "read /sys/fs/cgroup/n17.service/cgroup.procs",
                "write /sys/fs/cgroup/n17.service/manager/cgroup.procs 12",
                "write /sys/fs/cgroup/n17.service/manager/cgroup.procs 34",
                "read /sys/fs/cgroup/n17.service/cgroup.procs",
                "write /sys/fs/cgroup/n17.service/cgroup.subtree_control +cpu +memory +pids",
            ]
        );
        let mut bounds = limits();
        bounds.memory_bytes = 999.try_into().unwrap();
        bounds.decode.memory_bytes = 123.try_into().unwrap();
        bounds.cpu_millicores = 2000.try_into().unwrap();
        io.calls.lock().unwrap().clear();
        let worker = group.worker("worker", &bounds, 16).unwrap();
        worker.attach(42).unwrap();
        worker.kill().unwrap();
        assert!(!worker.oom_killed().unwrap());
        worker.remove().unwrap();
        let base = root().join("worker");
        let mut expected = vec![format!("create {}", base.display())];
        for (name, value) in [
            ("memory.max", "123"),
            ("memory.swap.max", "0"),
            ("memory.oom.group", "1"),
            ("pids.max", "16"),
            ("cpu.max", "200000 100000"),
        ] {
            expected.push(format!("write {} {value}", base.join(name).display()));
            expected.push(format!("read {}", base.join(name).display()));
        }
        for operation in [
            "probe cgroup.kill",
            "write cgroup.procs 42",
            "write cgroup.kill 1",
            "read memory.events",
            "read cgroup.events",
            "remove",
        ] {
            let (verb, rest) = operation.split_once(' ').unwrap_or((operation, ""));
            let (file, value) = rest.split_once(' ').unwrap_or((rest, ""));
            let path = if file.is_empty() {
                base.clone()
            } else {
                base.join(file)
            };
            expected.push(format!(
                "{verb} {}{}",
                path.display(),
                if value.is_empty() {
                    String::new()
                } else {
                    format!(" {value}")
                }
            ));
        }
        assert_eq!(*io.calls.lock().unwrap(), expected);
    }
    #[test]
    fn n17_delegation_ownership_controllers_and_each_effect_refuse() {
        for current in ["not unified", "0::/other.service\n", "0::/n17.slice\n"] {
            let io = Groups::new();
            io.files
                .lock()
                .unwrap()
                .insert(PathBuf::from("/proc/self/cgroup"), current.into());
            assert!(Delegation::with_io(root(), io).is_err());
        }
        for missing in ["cpu", "memory", "pids"] {
            let io = Groups::new();
            io.files.lock().unwrap().insert(
                root().join("cgroup.controllers"),
                ["cpu", "memory", "pids"]
                    .into_iter()
                    .filter(|item| *item != missing)
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            assert!(matches!(
                Delegation::with_io(root(), io),
                Err(Refusal::Unsupported)
            ));
        }
        for failure in 1..=8 {
            let io = Groups::new();
            *io.fail.lock().unwrap() = Some(failure);
            assert!(
                matches!(Delegation::with_io(root(), io), Err(Refusal::Unsupported)),
                "effect {failure}"
            );
        }
    }
    #[test]
    fn n17_worker_failure_rollback_and_event_error_priority() {
        for failure in 1..=12 {
            let io = Groups::new();
            let group = delegation(&io);
            io.calls.lock().unwrap().clear();
            *io.fail.lock().unwrap() = Some(failure);
            assert!(group.worker("worker", &limits(), 2).is_err());
            if failure > 1 {
                assert!(
                    io.calls
                        .lock()
                        .unwrap()
                        .last()
                        .unwrap()
                        .starts_with("remove ")
                );
            }
        }
        let io = Groups::new();
        let group = delegation(&io);
        io.calls.lock().unwrap().clear();
        *io.fail.lock().unwrap() = Some(2);
        io.events.lock().unwrap().push_back("populated 1".into());
        assert!(matches!(
            group.worker("worker", &limits(), 2),
            Err(Refusal::Cleanup)
        ));
        assert!(
            !io.calls
                .lock()
                .unwrap()
                .iter()
                .any(|call| call.starts_with("remove "))
        );
        *io.fail.lock().unwrap() = None;
    }
    #[test]
    fn n17_worker_events_and_effect_failures() {
        let io = Groups::new();
        let group = delegation(&io);
        let worker = group.worker("worker", &limits(), 2).unwrap();
        io.files.lock().unwrap().insert(
            root().join("worker/memory.events"),
            "low 0\noom_kill 2\n".into(),
        );
        assert!(worker.oom_killed().unwrap());
        io.events.lock().unwrap().extend([
            "populated 10".into(),
            "malformed".into(),
            "populated 0\n".into(),
        ]);
        assert!(!worker.empty().unwrap());
        assert!(!worker.empty().unwrap());
        assert!(worker.empty().unwrap());
        io.calls.lock().unwrap().clear();
        *io.fail.lock().unwrap() = Some(1);
        assert_eq!(worker.attach(42), Err(Refusal::Containment));
        io.calls.lock().unwrap().clear();
        assert_eq!(worker.kill(), Err(Refusal::Cleanup));
        io.calls.lock().unwrap().clear();
        assert_eq!(worker.empty(), Err(Refusal::Cleanup));
        io.calls.lock().unwrap().clear();
        assert_eq!(worker.oom_killed(), Err(Refusal::Cleanup));
        io.calls.lock().unwrap().clear();
        assert_eq!(worker.remove(), Err(Refusal::Cleanup));
    }
}
