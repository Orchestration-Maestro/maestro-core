use super::{Directory, open_nofollow};
#[cfg(windows)]
use std::{env, path::Component, sync::Barrier, thread};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process,
};
#[cfg(unix)]
use std::{
    io::Write,
    os::unix::fs::{PermissionsExt, symlink},
    sync::mpsc,
    thread,
    time::Duration,
};

/// A new empty directory whose name is unique within the test process.
pub(super) fn scratch() -> PathBuf {
    maestro_test_scratch::scratch_directory().unwrap()
}

/// `read_regular` on its own thread: the test fails, rather than hangs, if opening blocks.
#[cfg(unix)]
fn read_without_blocking(directory: Directory, name: &'static str) -> io::Result<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || sender.send(directory.read_regular(name)));
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("opening the artifact blocked")
}

/// `open_nofollow` on its own thread: the test fails, rather than hangs, if opening blocks.
#[cfg(unix)]
fn open_without_blocking(path: PathBuf) -> io::Result<fs::File> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || sender.send(open_nofollow(&path)));
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("opening the FIFO blocked")
}

#[cfg(unix)]
#[test]
fn directory_handles_resist_ancestor_replacement_and_reject_special_files() {
    let root = scratch();
    let output = root.join("output");
    let directory = Directory::open(&root, Path::new("output"), true).unwrap();
    fs::rename(&output, root.join("moved")).unwrap();
    fs::create_dir(root.join("outside")).unwrap();
    fs::write(root.join("outside/original.md"), "unchanged").unwrap();
    symlink(root.join("outside"), &output).unwrap();

    let mut original = directory.create_new("original.md").unwrap();
    original.write_all(b"safe").unwrap();
    drop(original);
    assert_eq!(directory.read_regular("original.md").unwrap(), b"safe");
    assert_eq!(fs::read(root.join("moved/original.md")).unwrap(), b"safe");
    assert_eq!(
        fs::read(root.join("outside/original.md")).unwrap(),
        b"unchanged"
    );
    assert!(Directory::open(&root, Path::new("output"), false).is_err());

    // POSIX's `mkfifo` utility: Linux and macOS both ship it.
    let made = process::Command::new("mkfifo")
        .arg(root.join("moved/fifo"))
        .status();
    assert!(made.unwrap().success());
    assert!(
        read_without_blocking(
            Directory::open(&root, Path::new("moved"), false).unwrap(),
            "fifo"
        )
        .is_err()
    );
    assert!(directory.read_regular(".").is_err());
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn only_directories_open_and_only_saving_creates_them() {
    let root = scratch();
    fs::write(root.join("plain"), "not a directory").unwrap();
    assert!(Directory::open(&root, Path::new("plain"), false).is_err());
    assert!(Directory::open(&root, Path::new("absent/deeper"), false).is_err());
    assert!(!root.join("absent").exists());
    drop(Directory::open(&root, Path::new("absent/deeper"), true).unwrap());
    assert!(root.join("absent/deeper").is_dir());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn artifacts_are_never_read_through_a_link() {
    let root = scratch();
    fs::write(root.join("target.md"), "linked").unwrap();
    #[cfg(unix)]
    symlink(root.join("target.md"), root.join("original.md")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    #[cfg(unix)]
    assert!(directory.read_regular("original.md").is_err());
    assert_eq!(directory.read_regular("target.md").unwrap(), b"linked");
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn read_regular_refuses_directories() {
    let root = scratch();
    fs::create_dir(root.join("directory")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert_eq!(
        directory.read_regular("directory").unwrap_err().to_string(),
        "artifact is not a regular file"
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn create_new_refuses_existing_names_and_planted_links() {
    let root = scratch();
    fs::write(root.join("existing"), "unchanged").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert_eq!(
        directory.create_new("existing").unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    #[cfg(unix)]
    {
        fs::write(root.join("target"), "unchanged").unwrap();
        symlink(root.join("target"), root.join("planted")).unwrap();
        assert_eq!(
            directory.create_new("planted").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(root.join("target")).unwrap(), b"unchanged");
    }
    #[cfg(windows)]
    {
        fs::create_dir(root.join("target")).unwrap();
        let created = process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(root.join("planted"))
            .arg(root.join("target"))
            .status()
            .unwrap();
        assert!(created.success());
        assert!(directory.create_new("planted").is_err());
        assert!(root.join("target").is_dir());
    }
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn link_never_replaces_its_destination() {
    let root = scratch();
    fs::write(root.join("source"), "source bytes").unwrap();
    fs::write(root.join("destination"), "destination bytes").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert_eq!(
        directory.link("source", "destination").unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(root.join("source")).unwrap(), b"source bytes");
    assert_eq!(
        fs::read(root.join("destination")).unwrap(),
        b"destination bytes"
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remove_verified_removes_only_matching_regular_files() {
    let root = scratch();
    fs::write(root.join("file"), b"expected").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert_eq!(
        directory
            .remove_verified("../file", b"expected", None)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(fs::read(root.join("file")).unwrap(), b"expected");
    directory
        .remove_verified("file", b"expected", None)
        .unwrap();
    assert!(!root.join("file").exists());
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remove_verified_refuses_edited_or_replaced_bytes() {
    let root = scratch();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    fs::write(root.join("edited"), b"new bytes").unwrap();
    assert!(
        directory
            .remove_verified("edited", b"old bytes", None)
            .is_err()
    );
    assert_eq!(fs::read(root.join("edited")).unwrap(), b"new bytes");

    fs::write(root.join("original"), b"old bytes").unwrap();
    fs::rename(root.join("original"), root.join("saved")).unwrap();
    fs::write(root.join("original"), b"replacement").unwrap();
    assert!(
        directory
            .remove_verified("original", b"old bytes", None)
            .is_err()
    );
    assert_eq!(fs::read(root.join("original")).unwrap(), b"replacement");
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remove_verified_quarantine_never_replaces_a_planted_name() {
    let root = scratch();
    let name = "file";
    fs::write(root.join(name), b"expected").unwrap();
    for counter in 0..128 {
        fs::write(
            root.join(format!(
                ".{name}.maestro-quarantine-{}-{counter}",
                process::id()
            )),
            b"planted",
        )
        .unwrap();
    }
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    directory.remove_verified(name, b"expected", None).unwrap();
    assert!(!root.join(name).exists());
    for counter in 0..128 {
        assert_eq!(
            fs::read(root.join(format!(
                ".{name}.maestro-quarantine-{}-{counter}",
                process::id()
            )))
            .unwrap(),
            b"planted"
        );
    }
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn remove_created_refuses_bytes_written_after_quarantine() {
    let root = scratch();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let created = directory.create_new("file").unwrap();
    let mut writer = created.try_clone().unwrap();
    let result = directory.remove_created_with("file", &created, || {
        writer.write_all(b"late").unwrap();
    });
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("created file changed")
    );
    assert_eq!(fs::read(root.join("file")).unwrap(), b"late");
    assert!(fs::read_dir(&root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("maestro-quarantine")
    }));
    let neighbour = directory.create_new("neighbour").unwrap();
    assert!(directory.remove_created("neighbour", &neighbour).is_ok());
    assert!(!root.join("neighbour").exists());
    drop((created, writer, neighbour, directory));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remove_verified_restore_never_replaces_a_concurrently_recreated_name() {
    let root = scratch();
    fs::write(root.join("file"), b"old bytes").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let remove_result = directory.remove_verified_with("file", b"expected", None, || {
        fs::write(root.join("file"), b"recreated").unwrap();
    });
    assert!(remove_result.is_err());
    assert_eq!(fs::read(root.join("file")).unwrap(), b"recreated");
    assert!(fs::read_dir(&root).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("maestro-quarantine")
    }));
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remove_verified_refuses_directories_before_quarantine() {
    let root = scratch();
    fs::create_dir(root.join("directory")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert!(directory.remove_verified("directory", b"", None).is_err());
    assert!(root.join("directory").is_dir());
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn remove_verified_refuses_links_without_removing_the_target() {
    let root = scratch();
    fs::write(root.join("target"), b"expected").unwrap();
    symlink(root.join("target"), root.join("link")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert!(
        directory
            .remove_verified("link", b"expected", None)
            .is_err()
    );
    assert_eq!(fs::read(root.join("target")).unwrap(), b"expected");
    assert!(
        fs::symlink_metadata(root.join("link"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn remove_verified_refuses_reparse_points() {
    let root = scratch();
    fs::create_dir(root.join("target")).unwrap();
    let created = process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(root.join("junction"))
        .arg(root.join("target"))
        .status()
        .unwrap();
    assert!(created.success());
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert!(directory.remove_verified("junction", b"", None).is_err());
    assert!(root.join("junction").exists());
    assert!(root.join("target").is_dir());
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remove_file_removes_the_name() {
    let root = scratch();
    fs::write(root.join("file"), "contents").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    directory.remove_file("file").unwrap();
    assert!(!root.join("file").exists());
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn remove_file_removes_a_link_without_removing_its_target() {
    let root = scratch();
    fs::write(root.join("target"), "contents").unwrap();
    symlink(root.join("target"), root.join("link")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    directory.remove_file("link").unwrap();
    assert!(fs::symlink_metadata(root.join("link")).is_err());
    assert_eq!(fs::read(root.join("target")).unwrap(), b"contents");
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn open_nofollow_refuses_links_and_opens_fifos_without_blocking() {
    let root = scratch();
    fs::write(root.join("target"), "contents").unwrap();
    symlink(root.join("target"), root.join("link")).unwrap();
    assert!(open_nofollow(&root.join("link")).is_err());
    let made = process::Command::new("mkfifo")
        .arg(root.join("fifo"))
        .status();
    assert!(made.unwrap().success());
    assert!(open_without_blocking(root.join("fifo")).is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn concurrent_openers_both_create_and_open_the_same_directory() {
    let root = scratch();
    for index in 0..64 {
        let name = format!("child-{index}");
        let barrier = Barrier::new(2);
        let (left, right) = thread::scope(|scope| {
            let left = scope.spawn(|| {
                barrier.wait();
                Directory::open(&root, Path::new(&name), true)
            });
            let right = scope.spawn(|| {
                barrier.wait();
                Directory::open(&root, Path::new(&name), true)
            });
            (left.join().unwrap(), right.join().unwrap())
        });
        assert!(left.is_ok(), "left opener failed: {left:?}");
        assert!(right.is_ok(), "right opener failed: {right:?}");
    }
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn a_create_failure_is_not_retried_as_a_missing_directory() {
    let root = scratch();
    let locked = root.join("locked");
    fs::create_dir(&locked).unwrap();
    let account = env::var("USERNAME").unwrap();
    let denied = process::Command::new("icacls")
        .arg(&locked)
        .arg("/deny")
        .arg(format!("{account}:(AD)"))
        .status()
        .unwrap();
    assert!(denied.success());

    let privileges = super::windows_test_security::DisabledAclBypass::new().unwrap();
    assert!(
        privileges.both_disabled().unwrap(),
        "backup and restore privileges must be disabled before the ACL probe"
    );
    let error = Directory::open(&root, Path::new("locked/new"), true).unwrap_err();
    let restored = process::Command::new("icacls")
        .arg(&locked)
        .arg("/remove:d")
        .arg(&account)
        .status()
        .unwrap();
    assert!(restored.success());
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn a_verbatim_root_anchors_the_walk_and_parent_traversal_stays_refused() {
    let root = scratch();
    let resolved = fs::canonicalize(&root).unwrap();
    assert!(matches!(
        resolved.components().next(),
        Some(Component::Prefix(prefix)) if prefix.kind().is_verbatim()
    ));
    drop(Directory::open(&root, Path::new("deeper"), true).unwrap());
    assert!(root.join("deeper").is_dir());
    let error = Directory::open(&root.join("deeper/../deeper"), Path::new(""), false);
    assert_eq!(
        error.unwrap_err().to_string(),
        "snapshot path contains parent traversal"
    );
    for below in ["deeper/../deeper", "C:relative", "C:\\deeper"] {
        let error = Directory::open(&root, Path::new(below), false).unwrap_err();
        assert_eq!(
            error.to_string(),
            "snapshot path below its root holds more than names"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn a_directory_junction_is_never_followed() {
    let root = scratch();
    fs::create_dir(root.join("outside")).unwrap();
    // A junction, unlike a symbolic link, needs no privilege to create.
    let created = process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(root.join("linked"))
        .arg(root.join("outside"))
        .status()
        .unwrap();
    assert!(created.success());
    assert!(Directory::open(&root, Path::new("linked"), false).is_err());
    assert!(open_nofollow(&root.join("linked")).is_err());
    assert!(Directory::open(&root, Path::new("linked/deeper"), true).is_err());
    assert!(!root.join("outside/deeper").exists());
    drop(Directory::open(&root.join("linked"), Path::new(""), false).unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn a_directory_that_cannot_be_created_is_reported_as_such() {
    let root = scratch();
    fs::create_dir(root.join("locked")).unwrap();
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o500)).unwrap();
    let error = Directory::open(&root, Path::new("locked/new"), true).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_dir_all(root).unwrap();
}
