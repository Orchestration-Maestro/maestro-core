//! Refused controller readbacks and nonempty roots must not release a worker.
use super::{
    cgroup::Delegation,
    port::Refusal,
    test_support::{Groups, limits},
};
use std::path::PathBuf;

#[test]
fn n17_group_scope_suffix_and_nonempty_manager_root() {
    let root = PathBuf::from("/sys/fs/cgroup/n17.scope");
    let io = Groups::new();
    for (path, value) in [
        (PathBuf::from("/proc/self/cgroup"), "0::/n17.scope\n"),
        (root.join("cgroup.controllers"), "pids memory cpu"),
        (root.join("cgroup.procs"), ""),
    ] {
        io.files.lock().unwrap().insert(path, value.into());
    }
    assert!(Delegation::with_io(root, io).is_ok());
    let root = PathBuf::from("/sys/fs/cgroup/n17.service");
    let io = Groups::new();
    io.observations
        .lock()
        .unwrap()
        .insert(root.join("cgroup.procs"), "12\n".into());
    assert!(matches!(
        Delegation::with_io(root, io.clone()),
        Err(Refusal::Unsupported)
    ));
    assert!(
        !io.calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call.contains("subtree_control"))
    );
    let root = PathBuf::from("/sys/fs/cgroup/n17.slice");
    io.files
        .lock()
        .unwrap()
        .insert(PathBuf::from("/proc/self/cgroup"), "0::/n17.slice\n".into());
    assert!(matches!(
        Delegation::with_io(root, io),
        Err(Refusal::Configuration)
    ));
}
#[test]
fn n17_group_each_mismatched_readback_rolls_back_and_remove_failure_wins() {
    for name in [
        "memory.max",
        "memory.swap.max",
        "memory.oom.group",
        "pids.max",
        "cpu.max",
    ] {
        let io = Groups::new();
        let root = PathBuf::from("/sys/fs/cgroup/n17.service");
        let group = Delegation::with_io(root.clone(), io.clone()).unwrap();
        io.observations
            .lock()
            .unwrap()
            .insert(root.join("worker").join(name), "wrong".into());
        assert!(matches!(
            group.worker("worker", &limits(), 2),
            Err(Refusal::Unsupported)
        ));
        assert!(
            io.calls
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .starts_with("remove ")
        );
    }
    let io = Groups::new();
    let root = PathBuf::from("/sys/fs/cgroup/n17.service");
    let group = Delegation::with_io(root.clone(), io.clone()).unwrap();
    io.observations
        .lock()
        .unwrap()
        .insert(root.join("worker/memory.max"), "wrong".into());
    io.calls.lock().unwrap().clear();
    *io.fail.lock().unwrap() = Some(5);
    assert!(matches!(
        group.worker("worker", &limits(), 2),
        Err(Refusal::Cleanup)
    ));
}
