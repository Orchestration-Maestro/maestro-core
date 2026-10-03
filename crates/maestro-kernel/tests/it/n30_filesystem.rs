//! Protected local-overlay and durable replacement contracts.
use maestro_kernel::filesystem::{atomic_replace, protected_root};
use maestro_test_scratch::scratch_directory;
use std::fs;

#[test]
fn n30_atomic_replacement_exposes_whole_new_bytes() {
    let root = scratch_directory().unwrap();
    let target = root.join("pointer");
    fs::write(&target, b"old").unwrap();
    atomic_replace(&target, b"complete new state").unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"complete new state");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n30_non_directory_root_refuses() {
    let root = scratch_directory().unwrap();
    let file = root.join("file");
    fs::write(&file, b"synthetic").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(protected_root(&file).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn n30_symlink_root_refuses_without_masking() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let root = scratch_directory().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let link = root.with_extension("link");
    symlink(&root, &link).unwrap();
    assert!(protected_root(&root).is_ok());
    assert!(protected_root(&link).is_err());
    fs::remove_file(link).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn n30_writable_root_refuses_without_masking() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = scratch_directory().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(protected_root(&root).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn n30_foreign_owner_root_refuses_without_masking() {
    use std::{os::unix::fs::MetadataExt as _, path::Path};
    let root = scratch_directory().unwrap();
    // No chmod/chown privilege needed: /usr is non-writable, non-symlink and root-owned.
    let foreign = Path::new("/usr");
    assert_ne!(
        fs::metadata(&root).unwrap().uid(),
        fs::metadata(foreign).unwrap().uid()
    );
    assert!(protected_root(foreign).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn n30_windows_junction_root_refuses() {
    use std::process::Command;
    let root = scratch_directory().unwrap();
    let junction = root.with_extension("junction");
    let result = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&root)
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(protected_root(&junction).is_err());
    fs::remove_dir(junction).unwrap();
    fs::remove_dir_all(root).unwrap();
}
