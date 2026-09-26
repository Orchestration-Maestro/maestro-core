//! Fake tools for the binary's tests: shell scripts found first on the
//! `PATH`, each logging its call, so that no test reaches the network or the
//! user's own systemd manager. `systemctl` finds a user manager running, and
//! the service neither enabled nor running, unless a test takes the manager
//! away; `curl` serves [`SERVED`], which is not Qdrant's archive.

use super::support::Home;
use std::{
    env,
    ffi::OsString,
    fs::{self, Permissions},
    os::unix::fs::PermissionsExt as _,
    path::PathBuf,
};

/// What the fake `curl` serves, whatever it is asked for.
pub(crate) const SERVED: &[u8] = b"not the archive of Qdrant 1.19.1\n";

/// The fake tools, in a directory of the test's home.
pub(crate) struct Fakes(PathBuf);

impl Fakes {
    /// The fake tools, written in `home` unless they are already.
    pub(crate) fn in_home(home: &Home) -> Self {
        let fakes = Self(home.root().join("fakes"));
        fs::create_dir_all(&fakes.0).unwrap();
        fs::write(fakes.0.join("served"), SERVED).unwrap();
        fakes.script(
            "curl",
            &format!("cat '{}'", fakes.0.join("served").display()),
        );
        fakes.script(
            "systemctl",
            "case \"$2\" in\n\
               is-enabled) echo disabled; exit 1 ;;\n\
               is-active) echo inactive; exit 3 ;;\n\
             esac",
        );
        fakes
    }

    /// Makes `systemctl` answer as it does where no user manager runs: every
    /// call fails, as it cannot reach one. Setup's own tests use it, on the
    /// one platform setup installs on.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub(crate) fn without_user_manager(&self) {
        self.script(
            "systemctl",
            "echo 'Failed to connect to bus: No medium found' >&2\nexit 1",
        );
    }

    /// The `PATH` that finds the fake tools first, then the test's own.
    pub(crate) fn path(&self) -> OsString {
        let mut path = OsString::from(&self.0);
        path.push(":");
        path.push(env::var_os("PATH").unwrap_or_default());
        path
    }

    /// Each call of a fake tool so far, as `<tool> <arguments>`, which
    /// setup's own tests read, on the one platform setup installs on.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub(crate) fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.0.join("log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Writes the fake tool `name`: a shell script that logs its call, then
    /// runs `body`.
    fn script(&self, name: &str, body: &str) {
        let path = self.0.join(name);
        let log = self.0.join("log");
        let script = format!(
            "#!/bin/sh\necho \"{name} $*\" >> '{}'\n{body}\n",
            log.display()
        );
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o700)).unwrap();
    }
}
