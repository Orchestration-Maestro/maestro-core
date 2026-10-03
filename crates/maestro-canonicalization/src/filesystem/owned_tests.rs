//! Permanent control files use the same held filesystem boundary on both platforms.
use super::owned::{ControlFile, FileLock, LockMode, OwnedRoot, SystemFileLock};
use maestro_test_scratch::scratch_directory;
#[cfg(windows)]
use std::path::Path;
use std::{fs, io, path::PathBuf};

/// A private scratch root removed after its held handles are dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// Allocate a root with the platform's inherited private permissions.
    fn new() -> Self {
        Self(scratch_directory().unwrap())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn filesystem_controls_create_resume_reuse_and_never_truncate() {
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    assert_eq!(
        root.open_control(ControlFile::Access).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    assert!(root.ensure_control(ControlFile::Access).unwrap());
    fs::write(path.join(".access.guard"), b"permanent").unwrap();
    assert!(root.ensure_control(ControlFile::Writer).unwrap());
    for control in [ControlFile::Access, ControlFile::Writer] {
        assert!(!root.ensure_control(control).unwrap());
        let guard = root.open_control(control).unwrap();
        guard
            .lock_with(&SystemFileLock, LockMode::Shared, false)
            .unwrap();
    }
    assert_eq!(fs::read(path.join(".access.guard")).unwrap(), b"permanent");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        for name in [".access.guard", ".writer.guard"] {
            assert_eq!(
                fs::metadata(path.join(name)).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}

/// A platform adapter whose locking primitive is unsupported.
struct Unsupported;

impl FileLock for Unsupported {
    fn acquire(&self, _file: &fs::File, _mode: LockMode, _wait: bool) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[test]
fn filesystem_unsupported_lock_adapter_fails_closed() {
    let scratch = Scratch::new();
    let root = OwnedRoot::open(&scratch.0.join("owned"), true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    for mode in [LockMode::Shared, LockMode::Exclusive] {
        for wait in [false, true] {
            assert_eq!(
                guard
                    .lock_with(&Unsupported, mode, wait)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::Unsupported
            );
        }
    }
}

#[test]
fn filesystem_control_directory_and_hard_link_refuse_with_valid_neighbour() {
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    fs::create_dir(path.join(".writer.guard")).unwrap();
    assert!(root.ensure_control(ControlFile::Writer).is_err());
    assert!(root.open_control(ControlFile::Writer).is_err());
    fs::remove_dir(path.join(".writer.guard")).unwrap();
    fs::hard_link(path.join(".access.guard"), path.join(".writer.guard")).unwrap();
    assert!(root.ensure_control(ControlFile::Writer).is_err());
    assert!(root.open_control(ControlFile::Writer).is_err());
    assert_eq!(fs::read(path.join(".access.guard")).unwrap(), b"");
}

#[cfg(unix)]
#[test]
fn filesystem_linked_controls_and_root_refuse_without_touching_neighbour() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let sentinel = scratch.0.join("sentinel");
    fs::write(&sentinel, b"outside").unwrap();
    symlink(&sentinel, path.join(".writer.guard")).unwrap();
    assert!(root.ensure_control(ControlFile::Writer).is_err());
    assert!(root.open_control(ControlFile::Writer).is_err());
    symlink(&path, scratch.0.join("linked")).unwrap();
    assert!(OwnedRoot::open(&scratch.0.join("linked"), false).is_err());
    assert_eq!(fs::read(&sentinel).unwrap(), b"outside");
    assert_eq!(fs::read(path.join(".access.guard")).unwrap(), b"");
}

#[cfg(unix)]
#[test]
fn filesystem_relocated_root_refuses_open_create_and_lock() {
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    fs::rename(&path, scratch.0.join("relocated")).unwrap();
    let replacement = OwnedRoot::open(&path, true).unwrap();
    replacement.ensure_control(ControlFile::Access).unwrap();
    assert!(root.open_control(ControlFile::Access).is_err());
    assert!(root.ensure_control(ControlFile::Writer).is_err());
    assert!(
        guard
            .lock_with(&SystemFileLock, LockMode::Shared, false)
            .is_err()
    );
    assert!(!path.join(".writer.guard").exists());
    assert!(!scratch.0.join("relocated/.writer.guard").exists());
}

#[cfg(unix)]
#[test]
fn filesystem_shared_control_permissions_refuse_without_repair() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    root.ensure_control(ControlFile::Writer).unwrap();
    fs::set_permissions(
        path.join(".writer.guard"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    assert!(root.open_control(ControlFile::Writer).is_err());
    assert!(root.ensure_control(ControlFile::Writer).is_err());
    assert!(root.open_control(ControlFile::Access).is_ok());
    assert_eq!(
        fs::metadata(path.join(".writer.guard"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
}

#[cfg(windows)]
#[test]
fn filesystem_windows_holds_root_and_inherits_read_only_safe_acl() {
    let scratch = Scratch::new();
    windows_acl(
        &scratch.0,
        r"
$ErrorActionPreference = 'Stop'
$user = [Security.Principal.WindowsIdentity]::GetCurrent().User
$acl = [Security.AccessControl.DirectorySecurity]::new()
$acl.SetOwner($user)
$acl.SetAccessRuleProtection($true, $false)
$acl.SetAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
    $user, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow'))
Set-Acl -LiteralPath $env:MAESTRO_TEST_ACL_ROOT -AclObject $acl
",
    );
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    root.ensure_control(ControlFile::Writer).unwrap();
    // Read-only inspection: control creation only inherits the private ACL.
    windows_acl(
        &path,
        r"
$ErrorActionPreference = 'Stop'
$user = [Security.Principal.WindowsIdentity]::GetCurrent().User
foreach ($name in @('.access.guard', '.writer.guard')) {
    $acl = Get-Acl -LiteralPath (Join-Path $env:MAESTRO_TEST_ACL_ROOT $name)
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $user.Value) {
        throw 'guard owner differs'
    }
    $rules = $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])
    if ($rules.Count -ne 1) { throw 'guard must inherit one private rule' }
    foreach ($rule in $rules) {
        if (!$rule.IsInherited -or $rule.IdentityReference.Value -ne $user.Value -or
            $rule.AccessControlType -ne 'Allow') { throw 'guard ACL is not private inherited' }
    }
}
",
    );
    assert!(fs::rename(&path, scratch.0.join("relocated")).is_err());
    let guard = root.open_control(ControlFile::Access).unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
    assert!(fs::remove_file(path.join(".access.guard")).is_err());
}

#[test]
fn filesystem_lock_adapter_preserves_shared_exclusive_and_wait_modes() {
    let scratch = Scratch::new();
    let root = OwnedRoot::open(&scratch.0.join("owned"), true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let first = root.open_control(ControlFile::Access).unwrap();
    let second = root.open_control(ControlFile::Access).unwrap();
    first
        .lock_with(&SystemFileLock, LockMode::Shared, true)
        .unwrap();
    second
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
    let contender = root.open_control(ControlFile::Access).unwrap();
    assert_eq!(
        contender
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(first);
    drop(second);
    contender
        .lock_with(&SystemFileLock, LockMode::Exclusive, true)
        .unwrap();
    let reader = root.open_control(ControlFile::Access).unwrap();
    assert_eq!(
        reader
            .lock_with(&SystemFileLock, LockMode::Shared, false)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(contender);
    reader
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
}

#[cfg(unix)]
#[test]
fn filesystem_replaced_control_handle_cannot_enter_a_split_lock_domain() {
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    root.ensure_control(ControlFile::Writer).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    fs::rename(path.join(".access.guard"), path.join("old-control")).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    assert!(
        guard
            .lock_with(&SystemFileLock, LockMode::Shared, false)
            .is_err()
    );
    root.open_control(ControlFile::Writer)
        .unwrap()
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    assert_eq!(fs::read(path.join("old-control")).unwrap(), b"");
}

#[test]
fn filesystem_control_retains_its_root_after_the_factory_returns() {
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    drop(root);
    guard
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    let reopened = OwnedRoot::open(&path, false).unwrap();
    let competing = reopened.open_control(ControlFile::Access).unwrap();
    assert_eq!(
        competing
            .lock_with(&SystemFileLock, LockMode::Shared, false)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(guard);
    competing
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
}

#[cfg(windows)]
#[test]
fn filesystem_reparse_root_and_guard_refuse_with_valid_neighbour() {
    use std::process::Command;
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let outside = scratch.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"outside").unwrap();
    for link in [scratch.0.join("linked"), path.join(".writer.guard")] {
        assert!(
            Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&link)
                .arg(&outside)
                .status()
                .unwrap()
                .success()
        );
    }
    assert!(OwnedRoot::open(&scratch.0.join("linked"), false).is_err());
    assert!(root.open_control(ControlFile::Writer).is_err());
    assert!(root.ensure_control(ControlFile::Writer).is_err());
    assert!(root.open_control(ControlFile::Access).is_ok());
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"outside");
}

#[cfg(unix)]
#[test]
fn filesystem_shared_root_refuses_control_creation_and_locks_until_secured() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o750)).unwrap();
    assert!(root.ensure_control(ControlFile::Writer).is_err());
    assert!(
        guard
            .lock_with(&SystemFileLock, LockMode::Shared, false)
            .is_err()
    );
    assert!(!path.join(".writer.guard").exists());
    root.make_private().unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
    assert!(root.ensure_control(ControlFile::Writer).unwrap());
}

#[test]
fn filesystem_control_validation_rechecks_aliases_before_locking() {
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    root.ensure_control(ControlFile::Writer).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    let alias = scratch.0.join("alias");
    fs::hard_link(path.join(".access.guard"), &alias).unwrap();
    // Unsupported must not reach the adapter: filesystem validation refuses first.
    assert_eq!(
        guard
            .lock_with(&Unsupported, LockMode::Shared, false)
            .unwrap_err()
            .to_string(),
        "control file must be regular with one link"
    );
    root.open_control(ControlFile::Writer)
        .unwrap()
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
    fs::remove_file(alias).unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
}

#[cfg(windows)]
#[test]
fn filesystem_windows_validation_binds_root_and_control_identities() {
    use super::windows::Directory;
    let scratch = Scratch::new();
    let path = scratch.0.join("owned");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    root.ensure_control(ControlFile::Writer).unwrap();
    let resolved = fs::canonicalize(&path).unwrap();
    let directory = Directory::open_resolved(&resolved, false).unwrap();
    directory.validate_owned(&resolved).unwrap();
    let other = scratch.0.join("other");
    fs::create_dir(&other).unwrap();
    assert!(
        directory
            .validate_owned(&fs::canonicalize(other).unwrap())
            .is_err()
    );
    let access = directory.open_control(".access.guard", false).unwrap();
    let writer = directory.open_control(".writer.guard", false).unwrap();
    directory
        .validate_control(".access.guard", &access)
        .unwrap();
    assert!(
        directory
            .validate_control(".access.guard", &writer)
            .is_err()
    );
    assert!(directory.validate_control("missing", &access).is_err());
}

/// Fixture-only ACL setup and read-only inspection, never production permission repair.
#[cfg(windows)]
fn windows_acl(path: &Path, script: &str) {
    use std::process::Command;
    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env_remove("PSModulePath")
        .env("MAESTRO_TEST_ACL_ROOT", path)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "private inherited ACL fixture/check failed"
    );
}
