//! Session-independent repair commands never read an unadmitted project lock.

use super::{backup_restore::backup, support::Home};
use std::{fs, path::PathBuf};

fn unreadable_lock(home: &Home) -> PathBuf {
    let directory = home.root().join(".maestro");
    fs::create_dir(&directory).unwrap();
    let lock = directory.join("authoring.lock.json");
    fs::write(&lock, b"unadmitted synthetic lock: never read by repair").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&lock, fs::Permissions::from_mode(0o000)).unwrap();
    }
    lock
}

#[test]
fn restore_checks_corrupt_backup_before_unadmitted_lock_and_non_directory_data() {
    let source = Home::new();
    source.add_synthetic();
    let backup = backup(&source, "repair-lock-backup");
    fs::write(backup.join("manifest.json"), b"corrupt synthetic backup").unwrap();
    let target = Home::new();
    let data = target.root().join("data");
    fs::remove_dir_all(&data).unwrap();
    fs::write(&data, b"do not replace").unwrap();
    let argument = backup.display().to_string();
    let before = target.run_in(target.root(), &["--json", "restore", "--from", &argument]);
    assert_eq!(before.code, Some(2), "{before:?}");
    let lock = unreadable_lock(&target);
    let after = target.run_in(target.root(), &["--json", "restore", "--from", &argument]);
    assert_eq!((after.code, after.stderr), (before.code, before.stderr));
    assert_eq!(fs::read(&data).unwrap(), b"do not replace");
    assert!(lock.is_file());
}

#[test]
fn backup_and_status_do_not_read_unadmitted_project_lock() {
    let home = Home::new();
    home.add_synthetic();
    let lock = unreadable_lock(&home);
    let status = home.run_in(home.root(), &["--json", "status"]);
    assert_eq!(status.code, Some(0), "{status:?}");
    assert_eq!(status.json()["schema"], "maestro-cli/status/1");
    let destination = home.root().join("repair-lock-backup");
    let argument = destination.display().to_string();
    let backed_up = home.run_in(home.root(), &["--json", "backup", "--to", &argument]);
    assert_eq!(backed_up.code, Some(0), "{backed_up:?}");
    assert_eq!(backed_up.json()["schema"], "maestro-cli/backup/1");
    assert!(lock.is_file());
}
