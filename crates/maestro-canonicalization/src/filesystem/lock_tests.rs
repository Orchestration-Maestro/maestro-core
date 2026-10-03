//! Mode conversions release the old lock before acquiring the new one on every OS.
use super::{ControlFile, FileLock, LockMode, OwnedRoot, SystemFileLock};
use maestro_test_scratch::scratch_directory;
use std::{env, fs, io, path::Path, process::Command};

/// Simulate Windows refusing conversion of a still-locked handle on Unix too.
struct RequireUnlocked<'a>(&'a Path);

impl FileLock for RequireUnlocked<'_> {
    fn acquire(&self, file: &fs::File, mode: LockMode, wait: bool) -> io::Result<()> {
        let probe = fs::OpenOptions::new().read(true).write(true).open(self.0)?;
        probe.try_lock().map_err(io::Error::from)?;
        probe.unlock()?;
        SystemFileLock.acquire(file, mode, wait)
    }
}

#[test]
fn filesystem_lock_conversion_releases_before_acquiring() {
    let scratch = scratch_directory().unwrap();
    let root = OwnedRoot::open(&scratch.join("graph"), true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let path = root.resolved_path().unwrap().join(".access.guard");
    let guard = root.open_control(ControlFile::Access).unwrap();
    for wait in [false, true] {
        for mode in [LockMode::Exclusive, LockMode::Shared, LockMode::Exclusive] {
            guard
                .lock_with(&RequireUnlocked(&path), mode, wait)
                .unwrap();
        }
    }
    drop((guard, root));
    fs::remove_dir_all(scratch).unwrap();
}

/// One subprocess per synchronized probe: process completion acknowledges the held mode.
#[test]
fn filesystem_lock_process_probe() {
    let Ok(path) = env::var("MAESTRO_LOCK_PROBE_ROOT") else {
        return;
    };
    let root = OwnedRoot::open(Path::new(&path), false).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    let exclusive = env::var("MAESTRO_LOCK_PROBE_MODE").unwrap() == "exclusive";
    let mode = if exclusive {
        LockMode::Exclusive
    } else {
        LockMode::Shared
    };
    let result = guard.lock_with(&SystemFileLock, mode, false);
    if env::var("MAESTRO_LOCK_PROBE_BUSY").unwrap() == "true" {
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
    } else {
        result.unwrap();
    }
}

#[test]
fn filesystem_lock_mode_changes_are_visible_to_another_process() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("graph");
    let root = OwnedRoot::open(&path, true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    for mode in [LockMode::Exclusive, LockMode::Shared, LockMode::Exclusive] {
        guard.lock_with(&SystemFileLock, mode, false).unwrap();
        for requested in ["shared", "exclusive"] {
            assert!(
                Command::new(env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "filesystem::lock_tests::filesystem_lock_process_probe",
                        "--nocapture"
                    ])
                    .env("MAESTRO_LOCK_PROBE_ROOT", &path)
                    .env("MAESTRO_LOCK_PROBE_MODE", requested)
                    .env(
                        "MAESTRO_LOCK_PROBE_BUSY",
                        (mode == LockMode::Exclusive || requested == "exclusive").to_string()
                    )
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
    drop((guard, root));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn filesystem_failed_conversion_clears_exclusive_removal_authority() {
    struct Unsupported;
    impl FileLock for Unsupported {
        fn acquire(&self, _file: &fs::File, _mode: LockMode, _wait: bool) -> io::Result<()> {
            Err(io::ErrorKind::Unsupported.into())
        }
    }
    let scratch = scratch_directory().unwrap();
    let root = OwnedRoot::open(&scratch.join("graph"), true).unwrap();
    root.ensure_control(ControlFile::Access).unwrap();
    let guard = root.open_control(ControlFile::Access).unwrap();
    guard
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    assert_eq!(
        guard
            .lock_with(&Unsupported, LockMode::Shared, false)
            .unwrap_err()
            .kind(),
        io::ErrorKind::Unsupported
    );
    let name = format!("g{}.lbdb", "a".repeat(64));
    assert_eq!(
        guard
            .remove_receipt_file(&name, None)
            .unwrap_err()
            .to_string(),
        "receipt removal requires exclusive access guard"
    );
    let contender = root.open_control(ControlFile::Access).unwrap();
    contender
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    drop((contender, guard, root));
    fs::remove_dir_all(scratch).unwrap();
}
