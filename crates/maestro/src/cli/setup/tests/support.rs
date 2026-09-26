//! What the install tests share, and doctor's check of Qdrant with them: a
//! scratch home holding the kernel's data directory and a configuration
//! home, fake `curl`, `tar` and `systemctl` that log each call, and a small
//! release: an archive holding a stand-in `qdrant`, with the digests a pin
//! gives them.

use super::super::{release::Release, service::Layout, tools::Tools};
use maestro_kernel::artifact::Digest;
use std::{
    collections::BTreeMap,
    env,
    fs::{self, Permissions},
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{self, Command},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};

/// A new scratch directory, removed with everything in it when dropped:
/// `data/maestro` is the kernel's data directory, `config` the
/// configuration home, and `tools` holds the fake tools, their log and
/// the state of the fake user manager.
pub(in crate::cli) struct Home(PathBuf);

impl Home {
    pub(in crate::cli) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = env::temp_dir().join(format!(
            "maestro-cli-setup-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for directory in ["data/maestro", "config", "tools/state"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let home = Self(root);
        home.script("curl", &format!("cat '{}'", home.served().display()));
        home.script("tar", "exec tar \"$@\"");
        home.script(
            "systemctl",
            &format!(
                "state='{}'\n\
                 if [ -e \"$state/fail-$2\" ]; then\n\
                   echo 'Failed to connect to bus' >&2; exit 1\n\
                 fi\n\
                 case \"$2\" in\n\
                   is-enabled) if [ -e \"$state/enabled\" ]; then echo enabled; \
                   else echo disabled; exit 1; fi ;;\n\
                   is-active) if [ -e \"$state/active\" ]; then echo active; \
                   else echo inactive; exit 3; fi ;;\n\
                   enable) touch \"$state/enabled\" ;;\n\
                   restart) touch \"$state/active\" ;;\n\
                 esac",
                home.state().display()
            ),
        );
        home
    }

    /// The directory of the scratch home.
    pub(in crate::cli) fn root(&self) -> &Path {
        &self.0
    }

    /// The layout of the service in this home.
    pub(super) fn layout(&self) -> Layout {
        Layout::new(&self.0.join("data/maestro"), &self.0.join("config"))
    }

    /// The fake tools.
    pub(in crate::cli) fn tools(&self) -> Tools {
        let tools = self.0.join("tools");
        Tools {
            curl: tools.join("curl"),
            tar: tools.join("tar"),
            systemctl: tools.join("systemctl"),
        }
    }

    /// Makes the fake `curl` serve `bytes`.
    pub(super) fn serve(&self, bytes: &[u8]) {
        fs::write(self.served(), bytes).unwrap();
    }

    /// Makes the fake `curl` fail as curl does on a 404, having printed
    /// nothing on stdout.
    pub(super) fn refuse_downloads(&self) {
        self.script(
            "curl",
            "echo 'curl: (22) The requested URL returned error: 404' >&2\nexit 22",
        );
    }

    /// Makes the fake `systemctl` fail its `verb`, as one that cannot reach
    /// the user manager does.
    pub(super) fn fail_systemctl(&self, verb: &str) {
        fs::write(self.state().join(format!("fail-{verb}")), "").unwrap();
    }

    /// Makes the fake `systemctl` succeed at its `verb` again.
    pub(super) fn heal_systemctl(&self, verb: &str) {
        fs::remove_file(self.state().join(format!("fail-{verb}"))).unwrap();
    }

    /// Makes the fake user manager forget that the service is enabled and
    /// running.
    pub(super) fn stop_service(&self) {
        for fact in ["enabled", "active"] {
            fs::remove_file(self.state().join(fact)).unwrap();
        }
    }

    /// Each call of a fake tool so far, as `<tool> <arguments>`.
    pub(in crate::cli) fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.log())
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Forgets the calls so far.
    pub(super) fn forget_calls(&self) {
        drop(fs::remove_file(self.log()));
    }

    /// Every file and directory under `data` and `config`, with its bytes,
    /// its mode and when it was last modified: what a run that changes
    /// nothing leaves equal.
    pub(super) fn files(&self) -> BTreeMap<PathBuf, (Vec<u8>, u32, SystemTime)> {
        let mut files = BTreeMap::new();
        let mut pending = vec![self.0.join("data"), self.0.join("config")];
        while let Some(path) = pending.pop() {
            let metadata = fs::symlink_metadata(&path).unwrap();
            let bytes = if metadata.is_dir() {
                pending.extend(entries(&path));
                Vec::new()
            } else {
                fs::read(&path).unwrap()
            };
            let mode = metadata.permissions().mode();
            files.insert(path, (bytes, mode, metadata.modified().unwrap()));
        }
        files
    }

    /// Writes the fake tool `name`: a shell script that logs its call, then
    /// runs `body`.
    fn script(&self, name: &str, body: &str) {
        let path = self.0.join("tools").join(name);
        let script = format!(
            "#!/bin/sh\necho \"{name} $*\" >> '{}'\n{body}\n",
            self.log().display()
        );
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o700)).unwrap();
    }

    /// What the fake `curl` serves.
    fn served(&self) -> PathBuf {
        self.0.join("tools/served")
    }

    /// The log of the fake tools' calls.
    fn log(&self) -> PathBuf {
        self.0.join("tools/log")
    }

    /// The state of the fake user manager.
    fn state(&self) -> PathBuf {
        self.0.join("tools/state")
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// A release of a stand-in `qdrant`, packed as Qdrant's archive is: one
/// member, `qdrant`, in a gzipped tar.
pub(super) struct Fixture {
    /// The bytes of the binary its archive holds.
    pub(super) binary: Vec<u8>,
    /// The archive's SHA-256.
    pub(super) archive_sha256: String,
    /// The binary's SHA-256.
    pub(super) binary_sha256: String,
}

impl Fixture {
    /// The release, packed with the system's `tar` in `home`, whose fake
    /// `curl` then serves it.
    pub(super) fn new(home: &Home) -> Self {
        let packing = home.root().join("packing");
        fs::create_dir(&packing).unwrap();
        let binary = b"#!/bin/sh\necho 'a stand-in qdrant'\n".to_vec();
        fs::write(packing.join("qdrant"), &binary).unwrap();
        let archive = home.root().join("qdrant.tar.gz");
        let packed = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&packing)
            .arg("qdrant")
            .status()
            .unwrap();
        assert!(packed.success());
        let archive = fs::read(archive).unwrap();
        home.serve(&archive);
        Self {
            archive_sha256: Digest::of(&archive).as_str().to_owned(),
            binary_sha256: Digest::of(&binary).as_str().to_owned(),
            binary,
        }
    }

    /// The release that pins this archive and its binary.
    pub(super) fn release(&self) -> Release<'_> {
        Release {
            version: "1.19.1",
            archive: "https://example.org/qdrant-test.tar.gz",
            archive_sha256: &self.archive_sha256,
            binary_sha256: &self.binary_sha256,
        }
    }
}

/// The entries of the directory at `path`.
fn entries(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect()
}

/// The mode of the file at `path`, its permission bits only.
pub(super) fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}
