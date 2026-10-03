//! Owned Unix controls and receipts anchored to the shared held directory.
use crate::unix::Directory;
use rustix::fs::{AtFlags, Mode, OFlags, linkat, mkdirat, openat, unlinkat};
use std::{
    fs::{File, Metadata, Permissions},
    io,
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    path::Path,
};
/// Controls need writable Windows-compatible locks, refuse links and never leak on exec.
/// RDWR opens a FIFO without waiting for a peer; the regular-file check then refuses it.
const CONTROL_FLAGS: OFlags = OFlags::RDWR.union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);

/// New controls request owner-only read/write permissions; stricter umasks fail validation.
const CONTROL_PERMISSIONS: Mode = Mode::RUSR.union(Mode::WUSR);

/// Receipts refuse links and blocking FIFOs and never leak on exec; read-only is zero bits.
const RECEIPT_FLAGS: OFlags = OFlags::NOFOLLOW
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);

impl Directory {
    /// Refuse relocation by comparing the named root with its held identity.
    pub(crate) fn validate_owned(&self, path: &Path) -> io::Result<()> {
        let named = Self::open_resolved(path, false)?;
        let held = self.0.metadata()?;
        let current = named.0.metadata()?;
        if (held.dev(), held.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other("owned root was relocated or replaced"));
        }
        Ok(())
    }

    /// Locks and control creation require an owner-only lock domain.
    pub(crate) fn validate_private(&self) -> io::Result<()> {
        if self.0.metadata()?.mode() & 0o7777 != 0o700 {
            return Err(io::Error::other(
                "owned root must have mode 0700; run setup --yes",
            ));
        }
        Ok(())
    }

    /// Secure the held directory, never a subsequently reopened path.
    pub(crate) fn make_private(&self) -> io::Result<()> {
        self.0.set_permissions(Permissions::from_mode(0o700))
    }

    /// Open a permanent control file with no-follow and no truncation.
    pub(crate) fn open_control(&self, name: &str, create: bool) -> io::Result<File> {
        let mut flags = CONTROL_FLAGS;
        if create {
            // Exclusive creation already refuses a symlink, even with a missing target.
            flags.remove(OFlags::NOFOLLOW);
            flags.insert(OFlags::CREATE.union(OFlags::EXCL));
        }
        let file = File::from(openat(&self.0, name, flags, CONTROL_PERMISSIONS)?);
        validate_control_metadata(&file.metadata()?, self.0.metadata()?.uid())?;
        if create {
            file.sync_all()?;
            self.0.sync_all()?;
        }
        Ok(file)
    }

    /// Bind a held control to its current, safely reopened name.
    pub(crate) fn validate_control(&self, name: &str, file: &File) -> io::Result<()> {
        let named = self.open_control(name, false)?;
        let held = file.metadata()?;
        let current = named.metadata()?;
        if (held.dev(), held.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other("permanent control file was replaced"));
        }
        Ok(())
    }

    /// Create exactly one new private directory beneath the held parent.
    pub(crate) fn reserve_child(&self, name: &str) -> io::Result<Self> {
        mkdirat(&self.0, name, Mode::RWXU)?;
        self.0.sync_all()?;
        Ok(Self(
            File::from(openat(
                &self.0,
                name,
                // Fresh mkdirat under the private parent; no-follow validation
                // refuses unsafe swaps.
                OFlags::CLOEXEC,
                Mode::empty(),
            )?),
            self.1.join(name),
        ))
    }

    /// Install a closed single-link file with anchored link/unlink and directory durability.
    /// The application's permanent guards exclude supported concurrent name changes.
    pub(crate) fn install_from(&self, staging: &Self, name: &str) -> io::Result<()> {
        let held = staging.open_receipt_file(name)?;
        held.sync_all()?;
        let named = staging.open_receipt_file(name)?;
        let before = held.metadata()?;
        let current = named.metadata()?;
        if (before.dev(), before.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other("publication source was replaced"));
        }
        linkat(&staging.0, name, &self.0, name, AtFlags::empty())?;
        self.0.sync_all()?;
        // A failure here preserves the installed name and any remaining staging link.
        // The lifecycle reports the partial install, never retries the closed writer.
        staging.remove_file(name)?;
        staging.0.sync_all()?;
        self.0.sync_all()
    }

    /// Open only a regular single-link receipt, anchored below this root.
    pub(crate) fn open_receipt_file(&self, name: &str) -> io::Result<File> {
        let file = File::from(openat(&self.0, name, RECEIPT_FLAGS, Mode::empty())?);
        validate_receipt_metadata(&file.metadata()?, self.0.metadata()?.uid())?;
        Ok(file)
    }

    /// Recheck the held identity immediately before one anchored unlink, then sync.
    pub(crate) fn remove_receipt_file(
        &self,
        name: &str,
        expected: Option<&File>,
    ) -> io::Result<bool> {
        let named = match self.open_receipt_file(name) {
            Ok(named) => named,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.0.sync_all()?;
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        let held = expected
            .ok_or_else(|| io::Error::other("receipt file appeared after lookup"))?
            .metadata()?;
        let current = named.metadata()?;
        if (held.dev(), held.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other("receipt file identity was replaced"));
        }
        unlinkat(&self.0, name, AtFlags::empty())?;
        self.0.sync_all()?;
        Ok(true)
    }

    /// Check a receipt child through its held file identity, refusing hard-link aliases.
    pub(crate) fn check_regular(&self, name: &str) -> io::Result<()> {
        let file = self.open_regular(name)?;
        if file.metadata()?.nlink() != 1 {
            return Err(io::Error::other("receipt file must have one link"));
        }
        Ok(())
    }
}

/// Validate a control's held metadata against the held root's owner.
fn validate_control_metadata(metadata: &Metadata, root_owner: u32) -> io::Result<()> {
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(io::Error::other(
            "control file must be regular with one link",
        ));
    }
    if metadata.mode() & 0o7777 != 0o600 || metadata.uid() != root_owner {
        return Err(io::Error::other(
            "control file must have its root's owner and mode 0600",
        ));
    }
    Ok(())
}

/// Validate a receipt's held metadata against the held root's owner.
fn validate_receipt_metadata(metadata: &Metadata, root_owner: u32) -> io::Result<()> {
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.uid() != root_owner {
        return Err(io::Error::other(
            "receipt file must be regular, single-link and root-owned",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Directory;
    use maestro_test_scratch::scratch_directory;
    use rustix::io::{FdFlags, fcntl_getfd};
    use std::{
        env, fs,
        io::{self, Read, Write},
        os::unix::fs::symlink,
        path::Path,
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::Duration,
    };

    #[test]
    fn filesystem_owned_leaf_rejects_foreign_owner() {
        use std::os::unix::fs::MetadataExt;
        let scratch = scratch_directory().unwrap();
        let root = Directory::open_resolved(&fs::canonicalize(&scratch).unwrap(), false).unwrap();
        root.make_private().unwrap();
        let file = root.open_control("guard", true).unwrap();
        let metadata = file.metadata().unwrap();
        let owner = metadata.uid();
        let foreign = owner ^ 1;
        super::validate_control_metadata(&metadata, owner).unwrap();
        super::validate_receipt_metadata(&metadata, owner).unwrap();
        assert_eq!(
            super::validate_control_metadata(&metadata, foreign)
                .unwrap_err()
                .to_string(),
            "control file must have its root's owner and mode 0600"
        );
        assert_eq!(
            super::validate_receipt_metadata(&metadata, foreign)
                .unwrap_err()
                .to_string(),
            "receipt file must be regular, single-link and root-owned"
        );
        drop((file, root));
        fs::remove_dir_all(scratch).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn filesystem_removal_refuses_unsyncable_directory() {
        use rustix::fs::{Mode, OFlags, open};
        for present in [false, true] {
            let scratch = fs::canonicalize(scratch_directory().unwrap()).unwrap();
            let root = Directory(
                fs::File::from(
                    open(
                        &scratch,
                        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .unwrap(),
                ),
                scratch.clone(),
            );
            let name = "receipt";
            let expected = if present {
                fs::write(scratch.join(name), b"disposable").unwrap();
                Some(root.open_receipt_file(name).unwrap())
            } else {
                None
            };
            assert_eq!(
                root.remove_receipt_file(name, expected.as_ref())
                    .unwrap_err()
                    .raw_os_error(),
                Some(9)
            );
            // Unlink happened on the present path, but neither path may claim durability.
            assert!(!scratch.join(name).exists());
            drop((expected, root));
            fs::remove_dir_all(scratch).unwrap();
        }
    }

    #[test]
    fn filesystem_unix_resolved_root_refuses_alias_before_canonicalization() {
        let scratch = fs::canonicalize(scratch_directory().unwrap()).unwrap();
        let real = scratch.join("real");
        fs::create_dir_all(real.join("owned")).unwrap();
        let alias = scratch.join("alias");
        symlink(&real, &alias).unwrap();
        let raw = alias.join("owned");
        assert_eq!(
            Directory::open_resolved(&raw, false).unwrap_err().kind(),
            io::ErrorKind::NotADirectory
        );
        let resolved = fs::canonicalize(raw).unwrap();
        let root = Directory::open_resolved(&resolved, false).unwrap();
        root.validate_owned(&resolved).unwrap();
        drop(root);
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn filesystem_unix_controls_are_writable_and_handles_close_on_exec() {
        let scratch = fs::canonicalize(scratch_directory().unwrap()).unwrap();
        let root = Directory::open_resolved(&scratch, false).unwrap();
        root.make_private().unwrap();
        let mut created = root.open_control("guard", true).unwrap();
        assert!(fcntl_getfd(&created).unwrap().contains(FdFlags::CLOEXEC));
        created.write_all(b"created").unwrap();
        let mut reopened = root.open_control("guard", false).unwrap();
        assert!(fcntl_getfd(&reopened).unwrap().contains(FdFlags::CLOEXEC));
        reopened.write_all(b"updated").unwrap();
        assert_eq!(fs::read(scratch.join("guard")).unwrap(), b"updated");
        symlink(scratch.join("guard"), scratch.join("linked")).unwrap();
        assert!(root.open_control("linked", false).is_err());
        assert!(root.open_receipt_file("linked").is_err());
        let receipt = root.open_receipt_file("guard").unwrap();
        assert!(fcntl_getfd(&receipt).unwrap().contains(FdFlags::CLOEXEC));
        let child = root.reserve_child("stage").unwrap();
        assert!(fcntl_getfd(&child.0).unwrap().contains(FdFlags::CLOEXEC));
        drop((child, receipt, reopened, created, root));
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn filesystem_unix_exclusive_creation_refuses_dangling_symlinks() {
        let scratch = scratch_directory().unwrap();
        let root = crate::OwnedRoot::open(&scratch.join("graph"), true).unwrap();
        // Dangling symlink creation refuses without following or creating its target.
        symlink(scratch.join("missing"), scratch.join("graph/.access.guard")).unwrap();
        assert!(root.ensure_control(crate::ControlFile::Access).is_err());
        assert!(!scratch.join("missing").exists());
        drop(root);
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn filesystem_unix_child_identity_validation_refuses_links_and_files() {
        let scratch = fs::canonicalize(scratch_directory().unwrap()).unwrap();
        let parent = Directory::open_resolved(&scratch, false).unwrap();
        parent.make_private().unwrap();
        let child = parent.reserve_child("stage").unwrap();
        fs::rename(scratch.join("stage"), scratch.join("held")).unwrap();
        symlink(scratch.join("held"), scratch.join("stage")).unwrap();
        assert!(child.validate_owned(&scratch.join("stage")).is_err());
        fs::remove_file(scratch.join("stage")).unwrap();
        fs::write(scratch.join("stage"), b"not a directory").unwrap();
        assert!(child.validate_owned(&scratch.join("stage")).is_err());
        drop((child, parent));
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn filesystem_unix_fifo_actor() {
        let Ok(path) = env::var("MAESTRO_FIFO_ROOT") else {
            return;
        };
        let root = Directory::open_resolved(Path::new(&path), false).unwrap();
        if env::var("MAESTRO_FIFO_MODE").unwrap() == "control" {
            assert!(root.open_control("fifo", false).is_err());
        } else {
            assert!(root.open_receipt_file("fifo").is_err());
        }
        println!("REFUSED");
    }

    #[test]
    fn filesystem_unix_fifos_refuse_without_waiting_for_a_peer() {
        let scratch = fs::canonicalize(scratch_directory().unwrap()).unwrap();
        let root = Directory::open_resolved(&scratch, false).unwrap();
        root.make_private().unwrap();
        // macOS has no mkfifoat in rustix; the POSIX utility creates the same fixture on both OSes.
        assert!(
            Command::new("mkfifo")
                .args(["-m", "600"])
                .arg(scratch.join("fifo"))
                .status()
                .unwrap()
                .success()
        );
        for mode in ["control", "receipt"] {
            let mut child = Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "unix_owned::tests::filesystem_unix_fifo_actor",
                    "--nocapture",
                ])
                .env("MAESTRO_FIFO_ROOT", &scratch)
                .env("MAESTRO_FIFO_MODE", mode)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let mut output = child.stdout.take().unwrap();
            let (send, receive) = mpsc::channel();
            let reader = thread::spawn(move || {
                let mut text = String::new();
                output.read_to_string(&mut text).unwrap();
                send.send(text.lines().any(|line| line == "REFUSED"))
                    .unwrap();
            });
            // Liveness bound only: a missing NONBLOCK would wait forever for a FIFO peer.
            let refused = receive.recv_timeout(Duration::from_secs(10));
            if refused.is_err() {
                child.kill().unwrap();
            }
            let status = child.wait().unwrap();
            reader.join().unwrap();
            assert!(refused.unwrap(), "{mode}");
            assert!(status.success());
        }
        drop(root);
        fs::remove_dir_all(scratch).unwrap();
    }
}
