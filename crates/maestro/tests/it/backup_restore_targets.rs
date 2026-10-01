//! Restore refuses kernel-owned target entries and preserves existing contents.

use super::{
    backup_restore::{backup, first_artifact, manifest, restore, write_manifest},
    support::Home,
};
use std::{fs, path::PathBuf};

fn valid_backup(name: &str) -> (Home, PathBuf) {
    let source = Home::new();
    source.add_synthetic();
    let backup = backup(&source, name);
    (source, backup)
}

#[test]
fn restore_refuses_a_missing_empty_artifact_store() {
    let source = Home::bare();
    drop(source.database());
    let backup = backup(&source, "empty-artifact-backup");
    assert!(
        manifest(&backup)["artifacts"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    fs::remove_dir(backup.join("artifacts")).unwrap();

    let target = Home::new();
    let refused = restore(&target, &backup);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "", "{refused:?}");
    assert!(!target.data().join("kernel.sqlite3").exists());
    assert!(!target.data().join("artifacts").exists());
}

#[test]
fn restore_refuses_existing_kernel_database() {
    let (_source, backup) = valid_backup("occupied-database-backup");
    let target = Home::new();
    let database = target.data().join("kernel.sqlite3");
    fs::write(&database, b"keep existing database").unwrap();

    let refused = restore(&target, &backup);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "", "{refused:?}");
    assert_eq!(fs::read(database).unwrap(), b"keep existing database");
    assert!(!target.data().join("artifacts").exists());
}

#[test]
fn restore_refuses_leftover_kernel_wal_file() {
    let (_source, backup) = valid_backup("leftover-wal-backup");
    let target = Home::new();
    let wal = target.data().join("kernel.sqlite3-wal");
    fs::write(&wal, b"keep leftover WAL").unwrap();

    let refused = restore(&target, &backup);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "", "{refused:?}");
    assert_eq!(fs::read(wal).unwrap(), b"keep leftover WAL");
    assert!(!target.data().join("kernel.sqlite3").exists());
    assert!(!target.data().join("artifacts").exists());
}

#[test]
fn restore_refuses_leftover_kernel_shm_file() {
    let (_source, backup) = valid_backup("leftover-shm-backup");
    let target = Home::new();
    let shm = target.data().join("kernel.sqlite3-shm");
    fs::write(&shm, b"keep leftover SHM").unwrap();

    let refused = restore(&target, &backup);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "", "{refused:?}");
    assert_eq!(fs::read(shm).unwrap(), b"keep leftover SHM");
    assert!(!target.data().join("kernel.sqlite3").exists());
    assert!(!target.data().join("artifacts").exists());
}

#[test]
fn restore_refuses_existing_artifacts_directory() {
    let (_source, backup) = valid_backup("occupied-artifacts-backup");
    let target = Home::new();
    let artifacts = target.data().join("artifacts");
    fs::create_dir(&artifacts).unwrap();
    fs::write(artifacts.join("keep"), b"keep existing artifacts").unwrap();

    let refused = restore(&target, &backup);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "", "{refused:?}");
    assert_eq!(
        fs::read(artifacts.join("keep")).unwrap(),
        b"keep existing artifacts"
    );
    assert!(!target.data().join("kernel.sqlite3").exists());
}

#[test]
fn restore_refuses_an_artifact_omitted_from_the_manifest() {
    let (_source, backup) = valid_backup("omitted-artifact-backup");
    let mut value = manifest(&backup);
    let omitted = first_artifact(&value).to_owned();
    value["artifacts"].as_array_mut().unwrap().remove(0);
    fs::remove_file(backup.join(omitted)).unwrap();
    write_manifest(&backup, &value);

    let target = Home::new();
    let refused = restore(&target, &backup);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "", "{refused:?}");
    assert!(!target.data().join("kernel.sqlite3").exists());
    assert!(!target.data().join("artifacts").exists());
}
