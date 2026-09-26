//! The system's tools setup runs, rather than crates of its own: `curl`
//! downloads the archive over HTTPS into memory, `tar` unpacks the binary
//! from it, still in memory, and `systemctl --user` manages the unit. Each is
//! found on the `PATH` by its name, or where a test puts a fake of it.

use crate::cli::failure::Failure;
use std::{
    io::Write as _,
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
};

/// Where each tool setup runs is.
#[derive(Debug, Clone)]
pub(super) struct Tools {
    /// `curl`.
    pub(super) curl: PathBuf,
    /// `tar`.
    pub(super) tar: PathBuf,
    /// `systemctl`.
    pub(super) systemctl: PathBuf,
}

/// What `systemctl` answered.
#[derive(Debug)]
pub(super) struct Answer {
    /// Whether it exited with 0.
    pub(super) success: bool,
    /// What it printed on stdout, without the whitespace around it.
    pub(super) stdout: String,
    /// What it printed on stderr, without the whitespace around it.
    pub(super) stderr: String,
}

impl Tools {
    /// Each tool by its name, found on the `PATH` when it runs.
    pub(super) fn on_path() -> Self {
        Self {
            curl: PathBuf::from("curl"),
            tar: PathBuf::from("tar"),
            systemctl: PathBuf::from("systemctl"),
        }
    }

    /// The bytes at `url`, downloaded over HTTPS alone, redirects included,
    /// with TLS 1.2 or later; an answer other than a success fails. The
    /// download gives up after 30 s without a connection or 15 minutes in
    /// all, and past 256 MiB, eight times Qdrant's archive, so that neither a
    /// stalled connection nor an endless answer holds setup or its memory.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] naming `url` and what `curl` said, when it cannot
    /// run or fails.
    pub(super) fn fetch(&self, url: &str) -> Result<Vec<u8>, Failure> {
        let mut command = Command::new(&self.curl);
        command.args([
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--tlsv1.2",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--connect-timeout",
            "30",
            "--max-time",
            "900",
            "--max-filesize",
            "268435456",
            url,
        ]);
        let output = run(&mut command, None)?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(Failure::failed(format!(
                "cannot download {url}: {}",
                trimmed(&output.stderr)
            )))
        }
    }

    /// The bytes of `member` in the gzipped tar `archive`, unpacked in
    /// memory: nothing is written.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] with what `tar` said, when it cannot run or
    /// fails.
    pub(super) fn unpack(&self, archive: &[u8], member: &str) -> Result<Vec<u8>, Failure> {
        let mut command = Command::new(&self.tar);
        command.args(["-xzOf", "-", member]);
        let output = run(&mut command, Some(archive.to_vec()))?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(Failure::failed(format!(
                "cannot unpack {member} from the archive: {}",
                trimmed(&output.stderr)
            )))
        }
    }

    /// Runs `systemctl --user` with `arguments`, and returns what it
    /// answered, whether it succeeded or not.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when it cannot run at all.
    pub(super) fn systemctl(&self, arguments: &[&str]) -> Result<Answer, Failure> {
        let mut command = Command::new(&self.systemctl);
        command.arg("--user").args(arguments);
        let output = run(&mut command, None)?;
        Ok(Answer {
            success: output.status.success(),
            stdout: trimmed(&output.stdout),
            stderr: trimmed(&output.stderr),
        })
    }

    /// Runs `systemctl --user` with `arguments`, which must succeed.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] naming the command and what it said, when it
    /// cannot run or fails.
    pub(super) fn systemctl_must(&self, arguments: &[&str]) -> Result<(), Failure> {
        let answer = self.systemctl(arguments)?;
        if answer.success {
            Ok(())
        } else {
            Err(Failure::failed(format!(
                "systemctl --user {} failed: {}",
                arguments.join(" "),
                answer.stderr
            )))
        }
    }
}

/// Runs `command` to its end with `input` on its stdin, written on a thread
/// of its own so that neither pipe ever fills, and returns what it printed.
///
/// # Errors
///
/// [`Failure::Failed`] naming the program when it cannot be started or
/// waited for.
fn run(command: &mut Command, input: Option<Vec<u8>>) -> Result<Output, Failure> {
    let program = PathBuf::from(command.get_program());
    let cannot = |error| Failure::failed(format!("cannot run {}: {error}", program.display()));
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(cannot)?;
    let writer = child.stdin.take().zip(input).map(|(mut stdin, bytes)| {
        // A tool that stops reading early says why on its stderr; the
        // broken pipe says nothing more.
        thread::spawn(move || drop(stdin.write_all(&bytes)))
    });
    let output = child.wait_with_output().map_err(cannot)?;
    if let Some(writer) = writer {
        drop(writer.join());
    }
    Ok(output)
}

/// `bytes` as text, without the whitespace around it.
fn trimmed(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}
