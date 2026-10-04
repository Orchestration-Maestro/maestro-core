//! Review-named default-driver guard stimuli; no privileged containment claims.
use super::{
    cgroup::Delegation,
    elf::interpreter,
    launch::{prepare, verify_snapshot},
    port::{Refusal, RuntimeFile},
    scratch::recover_with,
    test_support::{Groups, directory, limits, pin, request},
};
use maestro_kernel::artifact::Digest;
use std::{fs, path::Path};

#[test]
fn n17_prepare_snapshot_verification_failure_preserves_refusal() {
    let parent = directory();
    let root = parent.join("root");
    fs::create_dir(&root).unwrap();
    let mut launch = request(&parent);
    assert!(matches!(
        prepare(&root, &mut launch, &mut 1000, |path, digest, maximum| {
            verify_snapshot(path, digest, maximum).unwrap();
            Err(Refusal::LaunchPin)
        }),
        Err(Refusal::LaunchPin)
    ));
    assert!(root.join("parser").is_file());
    assert!(!root.join("input").exists());
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn n17_r1_guard_prepare_pins_and_loader() {
    let parent = directory();
    for case in ["parser", "runtime", "loader"] {
        let root = parent.join(case);
        fs::create_dir(&root).unwrap();
        let mut launch = request(&parent);
        if case == "parser" {
            launch.parser.digest = Digest::of(b"wrong parser");
        } else {
            let mut runtime = RuntimeFile {
                path: "etc/fixture".into(),
                pinned: pin(Path::new("/etc/hostname")),
                executable: false,
            };
            if case == "runtime" {
                runtime.pinned.digest = Digest::of(b"wrong runtime");
            } else {
                launch.parser = pin(Path::new("/bin/true"));
                runtime.path = interpreter(&fs::read("/bin/true").unwrap())
                    .unwrap()
                    .unwrap();
                runtime.executable = true;
            }
            launch.runtime.push(runtime);
        }
        assert!(
            matches!(
                prepare(&root, &mut launch, &mut 10_000_000, verify_snapshot),
                Err(Refusal::LaunchPin)
            ),
            "{case}"
        );
    }
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn n17_r1_guard_readback_and_preparer_names() {
    let io = Groups::new();
    let root = Path::new("/sys/fs/cgroup/n17.service");
    let delegation = Delegation::with_io(root.to_owned(), io.clone()).unwrap();
    let limits = limits();
    for (name, value) in [
        ("memory.max", limits.memory_bytes.get().to_string()),
        ("memory.swap.max", "0".into()),
        ("memory.oom.group", "1".into()),
        ("pids.max", "2".into()),
        (
            "cpu.max",
            format!("{} 100000", limits.cpu_millicores.get() * 100),
        ),
    ] {
        io.observations
            .lock()
            .unwrap()
            .insert(root.join("worker").join(name), format!(" \t{value}\n"));
    }
    delegation
        .worker("worker", &limits, 2)
        .unwrap()
        .remove()
        .unwrap();
    for name in [".n17-prefix-only", ".foreign-preparing"] {
        let parent = directory();
        let owned = parent.join(name);
        fs::create_dir(&owned).unwrap();
        fs::write(owned.join("cgroup-path"), "owned receipt").unwrap();
        assert_eq!(
            recover_with(&parent, &|_| panic!("no group lookup")),
            Err(Refusal::Cleanup)
        );
        assert_eq!(
            fs::read_to_string(owned.join("cgroup-path")).unwrap(),
            "owned receipt"
        );
        fs::remove_dir_all(parent).unwrap();
    }
}
