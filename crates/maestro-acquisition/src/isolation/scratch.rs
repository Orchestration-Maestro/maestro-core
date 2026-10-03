//! Atomic owned scratch admission and crash recovery; no source bytes in preparation.
use super::port::Refusal;
use rustix::fs::{CWD, RenameFlags, renameat_with};
use std::{
    fs::{self, File, TryLockError},
    io::ErrorKind,
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
/// Recover only this host's owned directory namespace, without killing cgroups.
pub(super) fn recover(parent: &Path) -> Result<(), Refusal> {
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
            match lock.try_lock() {
                Ok(()) => fs::remove_dir_all(&path).map_err(|_| Refusal::Cleanup)?,
                Err(TryLockError::WouldBlock) => {}
                Err(_) => return Err(Refusal::Cleanup),
            }
            continue;
        }
        if !name.starts_with("n17-") {
            return Err(Refusal::Cleanup);
        }
        committed(&path)?;
    }
    Ok(())
}
/// Missing/empty recorded groups permit deletion; active or malformed receipts refuse.
fn committed(path: &Path) -> Result<(), Refusal> {
    let receipt = fs::read_to_string(path.join("cgroup-path")).map_err(|_| Refusal::Cleanup)?;
    let group = PathBuf::from(receipt);
    if !group.starts_with("/sys/fs/cgroup") {
        return Err(Refusal::Cleanup);
    }
    match fs::read_to_string(group.join("cgroup.events")) {
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
    use super::{Preparing, recover};
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
        recover(&parent).unwrap();
        assert!(
            preparing.path.is_dir(),
            "live preparer must not be deleted/refused"
        );
        let destination = parent.join("n17-live");
        preparing.commit(&destination).unwrap();
        recover(&parent).unwrap();
        assert!(!destination.exists());
        let partial = parent.join(".n17-partial-preparing");
        fs::create_dir(&partial).unwrap();
        fs::write(partial.join("cgroup-path"), "incomplete").unwrap();
        recover(&parent).unwrap();
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
