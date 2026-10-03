//! The sandbox a probe runs a host in: a cleared environment holding only
//! `PATH`, a temporary `HOME` and a temporary project, so the owner's real
//! configuration is never read or written.

use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::Read,
    path::PathBuf,
    process::{self, Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::Duration,
};

/// How long one host run may take: a Pi parent that starts a child agent
/// takes about ten seconds here; a hung host is killed and fails its probe.
const HOST_DEADLINE: Duration = Duration::from_mins(3);

/// What a finished host run printed and how it ended.
#[derive(Debug)]
pub(super) struct Ended {
    /// The exit code, `None` when killed.
    pub(super) code: Option<i32>,
    /// Everything printed on standard output.
    pub(super) stdout: String,
    /// Everything printed on standard error, also echoed on the test's own.
    pub(super) stderr: String,
}

/// A new temporary directory holding `home/` and `project/`, removed when
/// dropped.
pub(super) struct Sandbox(PathBuf);

impl Sandbox {
    /// An empty sandbox under the platform's temporary directory.
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = env::temp_dir().join(format!(
            "maestro-host-probe-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("home")).unwrap();
        fs::create_dir_all(root.join("project")).unwrap();
        Self(root)
    }

    /// The host's `HOME`.
    pub(super) fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    /// The project the host runs in.
    pub(super) fn project(&self) -> PathBuf {
        self.0.join("project")
    }

    /// Writes `text` at `relative` under the sandbox, creating directories.
    pub(super) fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    /// Runs `command` in the project with only this process's `PATH`, the
    /// sandbox's `HOME` and `settings` in its environment.
    pub(super) fn run(&self, command: Command, settings: &[(&str, &OsStr)]) -> Ended {
        let mut command = command;
        command.envs(settings.iter().copied());
        self.run_with_path(command, &env::var_os("PATH").unwrap_or_default())
    }

    pub(super) fn run_with_path(&self, mut command: Command, path: &OsStr) -> Ended {
        let explicit = command
            .get_envs()
            .filter_map(|(key, value)| Some((key.to_owned(), value?.to_owned())))
            .collect::<Vec<(OsString, OsString)>>();
        command
            .env_clear()
            .envs(explicit)
            .env("PATH", path)
            .env("HOME", self.home())
            .current_dir(self.project())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let stderr = drain(child.stderr.take().unwrap());
        let stdout = drain(child.stdout.take().unwrap());
        // Standard output closes when the host ends; a host that outlives the
        // deadline is killed, which closes it too.
        let stdout = stdout.recv_timeout(HOST_DEADLINE).unwrap_or_else(|_| {
            child.kill().unwrap();
            stdout.recv().unwrap()
        });
        let status = child.wait().unwrap();
        let stderr = stderr.recv().unwrap();
        eprint!("{stderr}");
        Ended {
            code: status.code(),
            stdout,
            stderr,
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Reads `pipe` to its end on a thread of its own and sends the text.
fn drain(mut pipe: impl Read + Send + 'static) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut text = String::new();
        pipe.read_to_string(&mut text).unwrap();
        sender.send(text).unwrap();
    });
    receiver
}
