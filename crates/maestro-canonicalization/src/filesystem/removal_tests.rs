//! Receipt-only removal keeps the permanent lock domain and unrelated bytes.
use super::{ControlFile, LockMode, OwnedRoot, SystemFileLock};
use maestro_test_scratch::scratch_directory;
use std::{fs, io};

fn name() -> String {
    format!("g{}.lbdb", "a".repeat(64))
}

#[test]
fn filesystem_removal_is_single_file_durable_and_idempotent() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    fs::write(path.join(name()), b"disposable").unwrap();
    fs::write(path.join("unrelated.wal"), b"keep exactly").unwrap();
    let file = root.receipt_file(&name()).unwrap().unwrap();
    #[cfg(unix)]
    {
        assert!(guard.remove_receipt_file(&name(), Some(&file)).unwrap());
        assert!(!guard.remove_receipt_file(&name(), Some(&file)).unwrap());
        assert!(root.receipt_file(&name()).unwrap().is_none());
    }
    #[cfg(windows)]
    assert_eq!(
        guard
            .remove_receipt_file(&name(), Some(&file))
            .unwrap_err()
            .kind(),
        io::ErrorKind::Unsupported
    );
    assert_eq!(
        fs::read(path.join("unrelated.wal")).unwrap(),
        b"keep exactly"
    );
    assert!(path.join(".access.guard").is_file());
    drop((file, guard, root));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn filesystem_removal_refuses_noncanonical_names_and_requires_exclusive_access() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    root.ensure_control(ControlFile::Writer).unwrap();
    for invalid in [
        "../outside",
        "projection.db",
        ".access.guard",
        "",
        "gA.lbdb",
    ] {
        assert!(root.receipt_file(invalid).is_err());
    }
    fs::write(path.join(name()), b"keep").unwrap();
    let file = root.receipt_file(&name()).unwrap().unwrap();
    for control in [ControlFile::Access, ControlFile::Writer] {
        let guard = root.open_control(control).unwrap();
        assert!(guard.remove_receipt_file(&name(), Some(&file)).is_err());
        guard
            .lock_with(&SystemFileLock, LockMode::Shared, false)
            .unwrap();
        assert!(guard.remove_receipt_file(&name(), Some(&file)).is_err());
    }
    let writer = root.open_control(ControlFile::Writer).unwrap();
    writer
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    assert!(writer.remove_receipt_file(&name(), Some(&file)).is_err());
    assert_eq!(fs::read(path.join(name())).unwrap(), b"keep");
    drop((writer, file, root));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn filesystem_removal_refuses_directory_and_hard_link_with_valid_neighbour() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    fs::create_dir(path.join(name())).unwrap();
    assert!(root.receipt_file(&name()).is_err());
    fs::remove_dir(path.join(name())).unwrap();
    fs::write(path.join("outside"), b"sentinel").unwrap();
    fs::hard_link(path.join("outside"), path.join(name())).unwrap();
    assert!(root.receipt_file(&name()).is_err());
    fs::remove_file(path.join(name())).unwrap();
    fs::write(path.join(name()), b"valid").unwrap();
    assert!(root.receipt_file(&name()).unwrap().is_some());
    assert_eq!(fs::read(path.join("outside")).unwrap(), b"sentinel");
    drop(root);
    fs::remove_dir_all(scratch).unwrap();
}

#[cfg(unix)]
#[test]
fn filesystem_removal_refuses_leaf_link_replacement_and_relocated_root() {
    use std::os::unix::fs::symlink;
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    fs::write(scratch.join("outside"), b"sentinel").unwrap();
    symlink(scratch.join("outside"), path.join(name())).unwrap();
    assert!(root.receipt_file(&name()).is_err());
    fs::remove_file(path.join(name())).unwrap();
    fs::write(path.join(name()), b"original").unwrap();
    let file = root.receipt_file(&name()).unwrap().unwrap();
    fs::rename(path.join(name()), path.join("old")).unwrap();
    fs::write(path.join(name()), b"replacement").unwrap();
    assert!(guard.remove_receipt_file(&name(), Some(&file)).is_err());
    assert_eq!(fs::read(path.join(name())).unwrap(), b"replacement");
    fs::rename(&path, scratch.join("relocated")).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(root.receipt_file(&name()).is_err());
    assert!(guard.remove_receipt_file(&name(), Some(&file)).is_err());
    assert_eq!(fs::read(scratch.join("outside")).unwrap(), b"sentinel");
    drop((file, guard, root));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn filesystem_removal_refuses_appearance_and_a_different_root() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    assert!(root.receipt_file(&name()).unwrap().is_none());
    fs::write(path.join(name()), b"new").unwrap();
    assert!(guard.remove_receipt_file(&name(), None).is_err());
    let other = OwnedRoot::open(&path, false).unwrap();
    let file = other.receipt_file(&name()).unwrap().unwrap();
    assert!(guard.remove_receipt_file(&name(), Some(&file)).is_err());
    assert_eq!(fs::read(path.join(name())).unwrap(), b"new");
    drop((file, other, guard, root));
    fs::remove_dir_all(scratch).unwrap();
}

#[cfg(windows)]
#[test]
fn filesystem_removal_windows_holds_leaf_against_identity_replacement() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    fs::write(path.join(name()), b"original").unwrap();
    let file = root.receipt_file(&name()).unwrap().unwrap();
    assert!(fs::rename(path.join(name()), path.join("replaced")).is_err());
    assert!(fs::remove_file(path.join(name())).is_err());
    assert_eq!(fs::read(path.join(name())).unwrap(), b"original");
    drop((file, root));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn filesystem_removal_rechecks_aliases_and_mode_downgrades() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    fs::write(path.join(name()), b"held identity").unwrap();
    let file = root.receipt_file(&name()).unwrap().unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    fs::hard_link(path.join(name()), scratch.join("alias")).unwrap();
    assert!(guard.remove_receipt_file(&name(), Some(&file)).is_err());
    fs::remove_file(scratch.join("alias")).unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
    assert!(guard.remove_receipt_file(&name(), Some(&file)).is_err());
    guard
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    let wrong_name = format!("g{}.lbdb", "b".repeat(64));
    assert!(guard.remove_receipt_file(&wrong_name, Some(&file)).is_err());
    #[cfg(unix)]
    assert!(guard.remove_receipt_file(&name(), Some(&file)).unwrap());
    #[cfg(windows)]
    assert_eq!(
        guard
            .remove_receipt_file(&name(), Some(&file))
            .unwrap_err()
            .kind(),
        io::ErrorKind::Unsupported
    );
    drop((file, guard, root));
    fs::remove_dir_all(scratch).unwrap();
}

#[cfg(windows)]
#[test]
fn filesystem_removal_windows_reparse_leaf_refuses_with_valid_neighbour() {
    use std::process::Command;
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    let outside = scratch.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"outside bytes").unwrap();
    assert!(
        Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(path.join(name()))
            .arg(&outside)
            .status()
            .unwrap()
            .success()
    );
    assert!(root.receipt_file(&name()).is_err());
    fs::remove_dir(path.join(name())).unwrap();
    fs::write(path.join(name()), b"valid neighbour").unwrap();
    assert!(root.receipt_file(&name()).unwrap().is_some());
    assert_eq!(
        fs::read(outside.join("sentinel")).unwrap(),
        b"outside bytes"
    );
    drop(root);
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn filesystem_removal_poisoned_control_fails_closed() {
    use super::FileLock;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct PanicLock;
    impl FileLock for PanicLock {
        fn acquire(&self, _file: &fs::File, _mode: LockMode, _wait: bool) -> io::Result<()> {
            panic!("test lock adapter panic");
        }
    }
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    fs::write(path.join(name()), b"keep after panic").unwrap();
    let file = root.receipt_file(&name()).unwrap().unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| guard.lock_with(
            &PanicLock,
            LockMode::Shared,
            false
        )))
        .is_err()
    );
    assert!(
        guard
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .is_err()
    );
    assert!(guard.remove_receipt_file(&name(), Some(&file)).is_err());
    assert_eq!(fs::read(path.join(name())).unwrap(), b"keep after panic");
    drop((file, guard, root));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn filesystem_receipt_basename_has_exact_lowercase_hex_boundaries() {
    use super::is_receipt_basename;
    for digit in ["0", "9", "a", "f"] {
        assert!(is_receipt_basename(&format!("g{}.lbdb", digit.repeat(64))));
    }
    for digit in ["/", "@", "A", "F", "g", ":", "é"] {
        assert!(!is_receipt_basename(&format!("g{}.lbdb", digit.repeat(64))));
    }
    for invalid in [
        format!("g{}.lbdb", "a".repeat(63)),
        format!("g{}.lbdb", "a".repeat(65)),
        format!("G{}.lbdb", "a".repeat(64)),
        format!("g{}.LBDB", "a".repeat(64)),
    ] {
        assert!(!is_receipt_basename(&invalid));
    }
}
