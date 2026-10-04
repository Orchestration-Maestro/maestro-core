//! The actual mount/capability sequence is driven with ordered kernel observations.
use super::{
    port::{Mount, Refusal, SandboxIo},
    sandbox::{capabilities_with, enforced, filesystem_with},
};
use landlock::RulesetStatus;
use nix::mount::MntFlags;
use rustix::thread::{CapabilitiesSecureBits, CapabilitySets};
use std::{cell::RefCell, io, path::Path};

#[derive(Default)]
struct Effects {
    events: RefCell<Vec<String>>,
    fail: Option<usize>,
    visible: bool,
}
impl Effects {
    fn record(&self, event: String) -> Result<(), Refusal> {
        let mut events = self.events.borrow_mut();
        events.push(event);
        if self.fail == Some(events.len()) {
            Err(Refusal::Unsupported)
        } else {
            Ok(())
        }
    }
}
impl SandboxIo for Effects {
    fn mount(&self, op: Mount<'_>) -> Result<(), Refusal> {
        self.record(format!(
            "mount:{:?}:{}:{:?}:{:x}:{:?}",
            op.source,
            op.target.display(),
            op.filesystem,
            op.flags.bits(),
            op.options
        ))
    }
    fn pivot(&self, root: &Path, old: &Path) -> Result<(), Refusal> {
        self.record(format!("pivot:{}:{}", root.display(), old.display()))
    }
    fn chdir(&self, path: &Path) -> Result<(), Refusal> {
        self.record(format!("chdir:{}", path.display()))
    }
    fn unmount(&self, path: &Path, flags: MntFlags) -> Result<(), Refusal> {
        self.record(format!("unmount:{}:{:x}", path.display(), flags.bits()))
    }
    fn securebits(&self, bits: CapabilitiesSecureBits) -> Result<(), Refusal> {
        self.record(format!("securebits:{:x}", bits.bits()))
    }
    fn clear_ambient(&self) -> Result<(), Refusal> {
        self.record("ambient:clear".into())
    }
    fn capabilities(&self, sets: CapabilitySets) -> Result<(), Refusal> {
        assert!(sets.effective.is_empty());
        assert!(sets.permitted.is_empty());
        assert!(sets.inheritable.is_empty());
        self.record("capabilities:empty".into())
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        assert_eq!(path, Path::new("/sys/fs/cgroup/cgroup.procs"));
        if self.visible {
            Ok(b"visible".to_vec())
        } else {
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }
}
#[test]
fn n17_sandbox_mount_paths_options_flags_order_and_each_failure() {
    let root = Path::new("/view");
    let fx = Effects::default();
    assert_eq!(filesystem_with(root, 123, &fx), Ok(()));
    assert_eq!(
        *fx.events.borrow(),
        [
            "mount:None:/:None:44000:None",
            "mount:Some(\"/view\"):/view:None:1000:None",
            "mount:Some(\"/view/input\"):/view/input:None:1000:None",
            "mount:None:/view/input:None:102f:None",
            "mount:Some(\"tmpfs\"):/view/work:Some(\"tmpfs\"):e:Some(\"size=123,mode=0700\")",
            "pivot:/view:/view/old-root",
            "chdir:/",
            "unmount:/old-root:2",
            "mount:None:/:None:1027:None",
            "chdir:/work"
        ]
    );
    for step in 1..=10 {
        let fx = Effects {
            fail: Some(step),
            ..Effects::default()
        };
        assert_eq!(filesystem_with(root, 123, &fx), Err(Refusal::Unsupported));
        assert_eq!(fx.events.borrow().len(), step);
    }
}
#[test]
fn n17_sandbox_capabilities_securebits_empty_sets_visibility_and_failures() {
    let fx = Effects::default();
    assert_eq!(capabilities_with(&fx), Ok(()));
    assert_eq!(
        *fx.events.borrow(),
        ["securebits:3", "ambient:clear", "capabilities:empty"]
    );
    for step in 1..=3 {
        let fx = Effects {
            fail: Some(step),
            ..Effects::default()
        };
        assert_eq!(capabilities_with(&fx), Err(Refusal::Unsupported));
        assert_eq!(fx.events.borrow().len(), step);
    }
    let fx = Effects {
        visible: true,
        ..Effects::default()
    };
    assert_eq!(capabilities_with(&fx), Err(Refusal::Containment));
}
#[test]
fn n17_sandbox_landlock_requires_hard_fully_enforced_result() {
    assert_eq!(enforced(&RulesetStatus::FullyEnforced), Ok(()));
    assert_eq!(
        enforced(&RulesetStatus::PartiallyEnforced),
        Err(Refusal::Unsupported)
    );
    assert_eq!(
        enforced(&RulesetStatus::NotEnforced),
        Err(Refusal::Unsupported)
    );
}

#[test]
fn n17_landlock_path_failures_refuse_before_restricting_the_runner() {
    use super::sandbox::landlock;
    use maestro_test_scratch::scratch_directory;
    use std::fs;
    let root = scratch_directory().unwrap().join("root");
    assert_eq!(landlock(&root, None), Err(Refusal::Containment));
    fs::create_dir(&root).unwrap();
    assert_eq!(landlock(&root, None), Err(Refusal::Containment));
    fs::create_dir(root.join("work")).unwrap();
    assert_eq!(landlock(&root, None), Err(Refusal::Containment));
    fs::write(root.join("parser"), "synthetic").unwrap();
    assert_eq!(
        landlock(&root, Some("absent-loader")),
        Err(Refusal::Containment)
    );
    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}
