//! Atomic owned scratch admission and crash recovery; no source bytes in preparation.
use super::port::Refusal;
use rustix::fs::{CWD, RenameFlags, renameat_with};
use std::{
    fs::{self, File, TryLockError},
    io::{ErrorKind, Result as IoResult},
    path::{Path, PathBuf},
};

/// A held directory lock distinguishes a live preparer from SIGKILL leftovers.
#[derive(Debug)]
pub(super) struct Preparing {
    /// Same-parent staging directory; contains only the trusted receipt.
    path: PathBuf,
    /// Kernel releases this exclusive lock even on abrupt supervisor death.
    lock: File,
}
impl Preparing {
    /// Write a complete receipt under lock, before any snapshots/worker exist.
    pub(super) fn new(parent: &Path, name: &str, group: &Path) -> Result<Self, Refusal> {
        let path = parent.join(format!(".{name}-preparing"));
        fs::create_dir(&path).map_err(|_| Refusal::Cleanup)?;
        let result = (|| {
            let lock = File::open(&path).map_err(|_| Refusal::Cleanup)?;
            lock.try_lock().map_err(|_| Refusal::Cleanup)?;
            fs::write(
                path.join("cgroup-path"),
                group.to_str().ok_or(Refusal::Configuration)?,
            )
            .map_err(|_| Refusal::Cleanup)?;
            Ok(Self {
                path: path.clone(),
                lock,
            })
        })();
        if result.is_err() {
            fs::remove_dir_all(&path).map_err(|_| Refusal::Cleanup)?;
        }
        result
    }
    /// A no-replace atomic rename is the only transition that permits snapshots.
    pub(super) fn commit(self, destination: &Path) -> Result<(), Refusal> {
        let renamed = renameat_with(CWD, &self.path, CWD, destination, RenameFlags::NOREPLACE);
        if renamed.is_err() {
            fs::remove_dir_all(&self.path).map_err(|_| Refusal::Cleanup)?;
            return Err(Refusal::Cleanup);
        }
        drop(self.lock);
        Ok(())
    }
}
/// Narrow event observation seam; real private directories/locks/rename stay unchanged.
pub(super) fn recover_with(
    parent: &Path,
    events: &dyn Fn(&Path) -> IoResult<String>,
) -> Result<(), Refusal> {
    for entry in fs::read_dir(parent).map_err(|_| Refusal::Cleanup)? {
        let path = entry.map_err(|_| Refusal::Cleanup)?.path();
        if !fs::symlink_metadata(&path)
            .map_err(|_| Refusal::Cleanup)?
            .is_dir()
        {
            return Err(Refusal::Cleanup);
        }
        let name = path.file_name().ok_or(Refusal::Cleanup)?.to_string_lossy();
        if name.starts_with(".n17-") && name.ends_with("-preparing") {
            let lock = File::open(&path).map_err(|_| Refusal::Cleanup)?;
            reclaim(&path, &lock.try_lock())?;
            continue;
        }
        if !name.starts_with("n17-") {
            return Err(Refusal::Cleanup);
        }
        committed(&path, events)?;
    }
    Ok(())
}
/// A live preparer stays owned; every other lock error refuses recovery.
fn reclaim(path: &Path, locked: &Result<(), TryLockError>) -> Result<(), Refusal> {
    match locked {
        Ok(()) => fs::remove_dir_all(path).map_err(|_| Refusal::Cleanup),
        Err(TryLockError::WouldBlock) => Ok(()),
        Err(_) => Err(Refusal::Cleanup),
    }
}
/// Missing/empty recorded groups permit deletion; active or malformed receipts refuse.
fn committed(path: &Path, events: &dyn Fn(&Path) -> IoResult<String>) -> Result<(), Refusal> {
    let receipt = fs::read_to_string(path.join("cgroup-path")).map_err(|_| Refusal::Cleanup)?;
    let group = PathBuf::from(receipt);
    if !group.starts_with("/sys/fs/cgroup") {
        return Err(Refusal::Cleanup);
    }
    match events(&group.join("cgroup.events")) {
        Ok(events) if !events.lines().any(|line| line == "populated 0") => {
            return Err(Refusal::Cleanup);
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => return Err(Refusal::Cleanup),
    }
    fs::remove_dir_all(path).map_err(|_| Refusal::Cleanup)
}

#[cfg(test)]
mod tests {

    use super::{Preparing, recover_with};
    use maestro_test_scratch::scratch_directory;
    use std::{fs, path::Path};

    #[test]
    fn n17_atomic_preparation_live_lock_partial_recovery_and_no_replace() {
        let parent = scratch_directory().unwrap();
        fs::create_dir_all(&parent).unwrap();
        let preparing = Preparing::new(
            &parent,
            "n17-live",
            Path::new("/sys/fs/cgroup/nonexistent-n17-worker"),
        )
        .unwrap();
        recover_with(&parent, &|path| fs::read_to_string(path)).unwrap();
        assert!(
            preparing.path.is_dir(),
            "live preparer must not be deleted/refused"
        );
        let destination = parent.join("n17-live");
        preparing.commit(&destination).unwrap();
        recover_with(&parent, &|path| fs::read_to_string(path)).unwrap();
        assert!(!destination.exists());
        let partial = parent.join(".n17-partial-preparing");
        fs::create_dir(&partial).unwrap();
        fs::write(partial.join("cgroup-path"), "incomplete").unwrap();
        recover_with(&parent, &|path| fs::read_to_string(path)).unwrap();
        assert!(!partial.exists());
        let preparing = Preparing::new(
            &parent,
            "n17-collision",
            Path::new("/sys/fs/cgroup/nonexistent-n17-worker"),
        )
        .unwrap();
        let destination = parent.join("n17-collision");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("canary"), "original").unwrap();
        assert!(preparing.commit(&destination).is_err());
        assert_eq!(
            fs::read_to_string(destination.join("canary")).unwrap(),
            "original"
        );
        fs::remove_dir_all(parent).unwrap();
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::{Preparing, Refusal, recover_with};
    use crate::isolation::test_support::directory;
    use std::{ffi::OsString, fs::TryLockError, path::Path};
    use std::{
        fs, io,
        os::unix::{ffi::OsStringExt as _, fs::symlink},
        path::PathBuf,
    };

    #[test]
    fn n17_receipt_event_observations_preserve_active_malformed_and_io_errors() {
        for (index, event, removed) in [
            (0, Ok::<String, io::Error>("populated 0\n".into()), true),
            (1, Err(io::Error::from(io::ErrorKind::NotFound)), true),
            (2, Ok(String::new()), false),
            (3, Ok("populated 1".into()), false),
            (4, Ok("populated 00".into()), false),
            (
                5,
                Err(io::Error::from(io::ErrorKind::PermissionDenied)),
                false,
            ),
        ] {
            let parent = directory();
            let path = parent.join(format!("n17-{index}"));
            fs::create_dir(&path).unwrap();
            fs::write(
                path.join("cgroup-path"),
                "/sys/fs/cgroup/n17.service/worker",
            )
            .unwrap();
            let result = recover_with(&parent, &|group| {
                assert_eq!(
                    group,
                    Path::new("/sys/fs/cgroup/n17.service/worker/cgroup.events")
                );
                match &event {
                    Ok(text) => Ok(text.clone()),
                    Err(error) => Err(error.kind().into()),
                }
            });
            assert_eq!(
                result,
                if removed {
                    Ok(())
                } else {
                    Err(Refusal::Cleanup)
                }
            );
            assert_eq!(path.exists(), !removed);
            fs::remove_dir_all(parent).unwrap();
        }
    }
    #[test]
    fn n17_scratch_unowned_non_directory_bad_receipt_and_failed_preparation() {
        for name in [
            "foreign",
            "n17-file",
            "n17-symlink",
            "n17-bad-receipt",
            "n17-no-receipt",
        ] {
            let parent = directory();
            let path = parent.join(name);
            match name {
                "n17-file" => fs::write(&path, "not a directory").unwrap(),
                "n17-symlink" => symlink("/", &path).unwrap(),
                _ => {
                    fs::create_dir(&path).unwrap();
                }
            }
            if name == "n17-bad-receipt" {
                fs::write(path.join("cgroup-path"), "/not/owned").unwrap();
            }
            assert_eq!(
                recover_with(&parent, &|_| panic!("must refuse before group I/O")),
                Err(Refusal::Cleanup)
            );
            assert!(fs::symlink_metadata(&path).is_ok());
            fs::remove_dir_all(parent).unwrap();
        }
        let parent = directory();
        let invalid = PathBuf::from(OsString::from_vec(vec![0xff]));
        assert!(matches!(
            Preparing::new(&parent, "n17-invalid", &invalid),
            Err(Refusal::Configuration)
        ));
        assert!(!parent.join(".n17-invalid-preparing").exists());
        assert_eq!(
            recover_with(&parent.join("absent"), &|_| panic!("no events")),
            Err(Refusal::Cleanup)
        );
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn n17_preparer_lock_errors_and_failed_removal_preserve_ownership() {
        let parent = directory();
        assert_eq!(
            super::reclaim(
                &parent,
                &Err(TryLockError::Error(io::ErrorKind::PermissionDenied.into(),))
            ),
            Err(Refusal::Cleanup)
        );
        assert!(parent.is_dir());
        assert_eq!(
            super::reclaim(&parent, &Err(TryLockError::WouldBlock)),
            Ok(())
        );
        assert!(parent.is_dir());
        assert_eq!(
            super::reclaim(&parent.join("missing"), &Ok(())),
            Err(Refusal::Cleanup)
        );
        fs::remove_dir_all(parent).unwrap();
    }
}
