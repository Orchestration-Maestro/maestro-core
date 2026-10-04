//! Unix boundary checks for filesystem mutation regressions.
use crate::root::leaf_name;
use crate::{Directory, unix::private_metadata};
use maestro_test_scratch::scratch_directory;
use rustix::process::getuid;
use std::{
    fs::{self, File, Permissions},
    io::ErrorKind,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};

#[test]
fn mount_roots_include_root_devices_and_drive_spellings_not_ordinary_children() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert!(!directory.is_mount_root().unwrap());
    assert!(
        Directory::open_canonical(Path::new("/"))
            .unwrap()
            .is_mount_root()
            .unwrap()
    );
    assert!(
        Directory::open_canonical(Path::new("/dev"))
            .unwrap()
            .is_mount_root()
            .unwrap()
    );
    // The drive spelling is tested independently of whether this host mounts drvfs.
    let drive = Directory(directory.0.try_clone().unwrap(), "/mnt/c".into());
    assert!(drive.is_mount_root().unwrap());
    drop((drive, directory));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn preference_and_prefix_reads_return_the_real_bytes() {
    let root = scratch_directory().unwrap();
    fs::set_permissions(&root, Permissions::from_mode(0o700)).unwrap();
    let path = root.join("preferences");
    fs::write(&path, b"settings").unwrap();
    fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert_eq!(
        directory.read_preferences("preferences", 8).unwrap(),
        b"settings"
    );
    assert_eq!(
        directory.read_regular_prefix("preferences", 3).unwrap(),
        b"sett"
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verify_named_refuses_a_replaced_directory() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new("child"), true).unwrap();
    directory.verify_named().unwrap();
    fs::rename(root.join("child"), root.join("moved")).unwrap();
    fs::create_dir(root.join("child")).unwrap();
    assert_eq!(
        directory.verify_named().unwrap_err().to_string(),
        "directory changed during discovery"
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn directory_sync_propagates_unsupported_handle_errors() {
    // A read-only special handle cannot be fsynced on Linux or Darwin.
    let expected = File::open("/dev/null")
        .unwrap()
        .sync_all()
        .unwrap_err()
        .raw_os_error();
    let directory = Directory(File::open("/dev/null").unwrap(), "/dev/null".into());
    assert_eq!(directory.sync().unwrap_err().raw_os_error(), expected);
}

#[test]
fn leaf_names_reject_every_non_normal_spelling() {
    leaf_name("ordinary").unwrap();
    for name in ["", "a/b", "a\\b", "a:b", ".", ".."] {
        let error = leaf_name(name).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidInput, "{name}");
        assert_eq!(error.to_string(), "not one normal file name");
    }
}

#[test]
fn verified_removal_rejects_unsafe_names_before_opening() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    fs::write(root.join("a\\b"), b"bytes").unwrap();
    for name in ["", "a/b", "a\\b", ".", ".."] {
        assert_eq!(
            directory
                .remove_verified(name, b"bytes", None)
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidInput,
            "{name}"
        );
    }
    assert_eq!(fs::read(root.join("a\\b")).unwrap(), b"bytes");
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verified_removal_requires_both_identity_components() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    fs::write(root.join("file"), b"bytes").unwrap();
    let metadata = fs::metadata(root.join("file")).unwrap();
    for identity in [
        (metadata.dev() ^ 1, metadata.ino()),
        (metadata.dev(), metadata.ino() ^ 1),
    ] {
        assert_eq!(
            directory
                .remove_verified("file", b"bytes", Some(identity))
                .unwrap_err()
                .to_string(),
            "verified removal refused: file bytes changed"
        );
        assert_eq!(fs::read(root.join("file")).unwrap(), b"bytes");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }
    directory
        .remove_verified("file", b"bytes", Some((metadata.dev(), metadata.ino())))
        .unwrap();
    assert!(!root.join("file").exists());
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn created_child_hardening_and_removal_keep_the_original_identity() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let child = directory.create_child("child").unwrap();
    drop(directory.harden_created_child("child", &child).unwrap());
    directory.remove_created_child("child", &child).unwrap();
    assert!(!root.join("child").exists());
    let file = directory.create_new("file").unwrap();
    fs::write(root.join("file"), b"written").unwrap();
    assert!(
        directory
            .remove_created_bytes("file", &file, b"different")
            .is_err()
    );
    assert_eq!(fs::read(root.join("file")).unwrap(), b"written");
    directory
        .remove_created_bytes("file", &file, b"written")
        .unwrap();
    assert!(!root.join("file").exists());
    drop((file, child, directory));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn drive_mount_accepts_only_single_ascii_drive_children() {
    for (name, expected) in [
        ("/mnt/a", true),
        ("/mnt/Z", true),
        ("/a", false),
        ("/mnt", false),
        ("/mnt/ab", false),
        ("/mnt/1", false),
        ("/mnt/é", false),
        ("/mnt/a/child", false),
    ] {
        // A shared root handle makes the spelling the only possible mount distinction.
        let directory = Directory(File::open("/").unwrap(), name.into());
        assert_eq!(directory.is_mount_root().unwrap(), expected, "{name}");
    }
}

#[test]
fn private_metadata_checks_owner_write_and_read_independently() {
    let root = scratch_directory().unwrap();
    let path = root.join("preferences");
    fs::write(&path, b"preferences").unwrap();
    let file = File::open(&path).unwrap();
    for mode in [0o600, 0o400, 0o640] {
        fs::set_permissions(&path, Permissions::from_mode(mode)).unwrap();
        private_metadata(&file).unwrap();
    }
    for mode in [0o620, 0o602, 0o200] {
        fs::set_permissions(&path, Permissions::from_mode(mode)).unwrap();
        let error = private_metadata(&file).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Other);
        assert_eq!(
            error.to_string(),
            "foreign-owned, other-writable or unreadable preferences"
        );
    }
    let foreign = File::open("/").unwrap();
    assert_ne!(foreign.metadata().unwrap().uid(), getuid().as_raw());
    assert!(private_metadata(&foreign).is_err());
    drop(file);
    fs::remove_dir_all(root).unwrap();
}
