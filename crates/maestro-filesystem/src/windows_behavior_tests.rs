//! Native Windows directory leases, reads and quarantine refusals.
use crate::{
    Directory,
    windows_security_fixture_tests::{icacls, native_owner},
    windows_test_security::DisabledAclBypass,
};
use maestro_test_scratch::scratch_directory;
use std::{
    env, fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
fn mount_root_distinguishes_drive_anchor_from_descendants() {
    let root = scratch_directory().unwrap().canonicalize().unwrap();
    let anchor: PathBuf = root
        .components()
        .take_while(|part| matches!(part, Component::Prefix(_) | Component::RootDir))
        .collect();
    let drive = Directory::open_canonical(&anchor).unwrap();
    let descendant = Directory::open_canonical(&root).unwrap();
    assert!(drive.is_mount_root().unwrap());
    assert!(!descendant.is_mount_root().unwrap());
    drop((drive, descendant));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn preferences_read_returns_bytes_and_refuses_a_regular_file_only_when_oversize() {
    let root = scratch_directory().unwrap();
    let owner = format!("*{}", native_owner(&root));
    icacls(
        &root,
        &["/inheritance:r", "/grant:r", &format!("{owner}:(OI)(CI)F")],
    );
    fs::write(root.join("preferences"), b"prefs").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let read = directory.read_preferences("preferences", 5);
    assert!(
        read.is_ok(),
        "regular private preferences must be readable: {read:?}"
    );
    assert_eq!(read.unwrap(), b"prefs");
    assert_eq!(
        directory
            .read_preferences("preferences", 4)
            .unwrap_err()
            .kind(),
        ErrorKind::FileTooLarge
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn regular_prefix_returns_the_limit_plus_one_sentinel() {
    let root = scratch_directory().unwrap();
    fs::write(root.join("file"), b"prefix bytes").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert_eq!(directory.read_regular_prefix("file", 3).unwrap(), b"pref");
    assert_eq!(
        directory.read_regular_prefix("file", 20).unwrap(),
        b"prefix bytes"
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verify_named_refuses_a_directory_that_can_no_longer_be_opened() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let owner = format!("*{}", native_owner(&root));
    let privileges = DisabledAclBypass::new().unwrap();
    assert!(privileges.both_disabled().unwrap());
    icacls(&root, &["/deny", &format!("{owner}:(RD)")]);
    let result = directory.verify_named();
    icacls(&root, &["/remove:d", &owner]);
    assert!(
        result.is_err(),
        "verification must reopen the denied directory"
    );
    assert_eq!(result.unwrap_err().kind(), ErrorKind::PermissionDenied);
    directory.verify_named().unwrap();
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verified_removal_refuses_slash_and_backslash_before_opening() {
    let root = scratch_directory().unwrap();
    fs::create_dir(root.join("nested")).unwrap();
    fs::write(root.join("nested/file"), b"keep").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    for name in ["nested/file", "nested\\file", "", ".", ".."] {
        assert_eq!(
            directory
                .remove_verified(name, b"keep", None)
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidInput,
            "{name:?} must be refused before quarantine"
        );
        assert_eq!(fs::read(root.join("nested/file")).unwrap(), b"keep");
    }
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verified_removal_refuses_a_directory_before_creating_quarantine() {
    let root = scratch_directory().unwrap();
    fs::create_dir(root.join("directory")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let result = directory.remove_verified("directory", b"", None);
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not a regular file")
    );
    assert!(root.join("directory").is_dir());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn created_child_rollback_actually_removes_the_empty_directory() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let created = directory.create_child("created").unwrap();
    directory.remove_created_child("created", &created).unwrap();
    drop(created);
    assert!(!root.join("created").exists());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn quarantine_permission_failure_returns_without_retry() {
    const CHILD: &str = "MAESTRO_QUARANTINE_REFUSAL_CHILD";
    if let Some(root) = env::var_os(CHILD) {
        let directory = Directory::open(Path::new(&root), Path::new(""), false).unwrap();
        let privileges = DisabledAclBypass::new().unwrap();
        assert!(privileges.both_disabled().unwrap());
        assert_eq!(
            directory
                .remove_verified("file", b"keep", None)
                .unwrap_err()
                .kind(),
            ErrorKind::PermissionDenied
        );
        return;
    }
    let root = scratch_directory().unwrap();
    fs::write(root.join("file"), b"keep").unwrap();
    let owner = format!("*{}", native_owner(&root));
    icacls(&root, &["/deny", &format!("{owner}:(AD)")]);
    let mut child = Command::new(env::current_exe().unwrap())
        .args([
            "--exact",
            "windows_behavior_tests::quarantine_permission_failure_returns_without_retry",
            "--nocapture",
        ])
        .env(CHILD, &root)
        .spawn()
        .unwrap();
    // Match the existing mutation-proof timeout: 10 seconds, terminate after three periods.
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        thread::sleep(Duration::from_millis(50));
    };
    icacls(&root, &["/remove:d", &owner]);
    assert_eq!(fs::read(root.join("file")).unwrap(), b"keep");
    fs::remove_dir_all(root).unwrap();
    assert!(
        status.is_some_and(|status| status.success()),
        "quarantine refusal must return PermissionDenied, not retry forever: {status:?}"
    );
}
