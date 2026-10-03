//! Guard-first health tests; process handshakes use pipes, never sleeps.

use super::{
    probe::{NativePlatform, Probe, ProbeReceipt},
    tests::Fixture,
};
use crate::graph::projection::health::ProbeError;
use maestro_filesystem::{ControlFile, FileLock, LockMode, OwnedRoot, SystemFileLock};
use maestro_kernel::{scope::Config, store::Database};
use std::path::Path;
use std::{
    env,
    fs::File,
    io::{self, BufRead as _, Write as _},
    process::{Child, ChildStdout, Command, Stdio},
};

pub(super) fn guards(fixture: &Fixture) -> OwnedRoot {
    let root = OwnedRoot::open(&fixture.path, false).unwrap();
    for control in [ControlFile::Access, ControlFile::Writer] {
        root.ensure_control(control).unwrap();
    }
    root
}

#[test]
fn graph_probe_process_holder() {
    let Some(path) = env::var_os("MAESTRO_PROBE_HOLDER") else {
        return;
    };
    let root = OwnedRoot::open(path.as_ref(), false).unwrap();
    let kind = env::var("MAESTRO_PROBE_WRITER").unwrap();
    if kind == "native" {
        let _native = native_writer(path.as_ref()).unwrap();
        signal_and_wait();
        return;
    }
    let control = if kind == "writer" {
        ControlFile::Writer
    } else {
        ControlFile::Access
    };
    let held = root.open_control(control).unwrap();
    held.lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    signal_and_wait();
    drop(held);
}

struct Holder(Child, io::BufReader<ChildStdout>);
impl Holder {
    fn new(fixture: &Fixture, kind: &str) -> Self {
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "graph::projection::engine::probe_tests::graph_probe_process_holder",
                "--nocapture",
            ])
            .env("MAESTRO_PROBE_HOLDER", &fixture.path)
            .env("MAESTRO_PROBE_WRITER", kind)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let mut output = io::BufReader::new(child.stdout.take().unwrap());
        loop {
            let mut line = String::new();
            assert_ne!(
                output.read_line(&mut line).unwrap(),
                0,
                "holder exited before acquiring lock"
            );
            if line.trim() == "READY" {
                break;
            }
        }
        Self(child, output)
    }
}
impl Drop for Holder {
    fn drop(&mut self) {
        writeln!(self.0.stdin.take().unwrap(), "RELEASE").unwrap();
        io::copy(&mut self.1, &mut io::sink()).unwrap();
        assert!(self.0.wait().unwrap().success());
    }
}

#[test]
fn graph_probe_process_locks_precede_missing_kernel_inventory() {
    for writer in [false, true] {
        let fixture = Fixture::new();
        let _root = guards(&fixture);
        let holder = Holder::new(&fixture, if writer { "writer" } else { "access" });
        let outcome = Probe::receipt(
            &fixture.path.join("missing"),
            &fixture.path,
            &Config::default(),
            &SystemFileLock,
            None,
        );
        let error = outcome.expect_err("process holder cannot be healthy");
        if writer {
            assert!(matches!(error, ProbeError::Locked(_)), "{error:?}");
        } else {
            assert_eq!(error, ProbeError::CleanupInProgress);
        }
        println!("captured guard contention: {error:?}");
        drop(holder);
        assert_eq!(
            Probe::receipt(
                &fixture.path.join("missing"),
                &fixture.path,
                &Config::default(),
                &SystemFileLock,
                None,
            )
            .err(),
            Some(ProbeError::AuthorityMissing)
        );
    }
}

struct Unsupported;
impl FileLock for Unsupported {
    fn acquire(&self, _: &File, mode: LockMode, wait: bool) -> io::Result<()> {
        assert!(matches!(mode, LockMode::Shared));
        assert!(!wait);
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[test]
fn graph_probe_missing_guards_and_unsupported_locking_fail_before_inventory() {
    let fixture = Fixture::new();
    assert_eq!(
        Probe::receipt(
            &fixture.path,
            &fixture.path,
            &Config::default(),
            &SystemFileLock,
            None,
        )
        .err(),
        Some(ProbeError::GuardUnavailable)
    );
    let _root = guards(&fixture);
    assert_eq!(
        Probe::receipt(
            &fixture.path,
            &fixture.path,
            &Config::default(),
            &Unsupported,
            None,
        )
        .err(),
        Some(ProbeError::LockUnavailable)
    );
    assert!(!fixture.path.join("kernel.sqlite3").exists());
}

#[test]
fn graph_probe_current_empty_authority_publishes_nothing() {
    let fixture = Fixture::new();
    let _root = guards(&fixture);
    drop(Database::open_in(&fixture.path).unwrap());
    assert!(matches!(
        Probe::receipt(
            &fixture.path,
            &fixture.path,
            &Config::default(),
            &SystemFileLock,
            None,
        )
        .unwrap(),
        ProbeReceipt::NonePublished
    ));
}

/// Handshake at the actual held lock, never a guessed delay.
fn signal_and_wait() {
    println!("READY");
    io::stdout().flush().unwrap();
    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap();
}

/// Test-only writable native contention capture; health never calls this helper.
fn native_writer(path: &Path) -> Result<lbug::Database, lbug::Error> {
    #[cfg(windows)]
    #[expect(
        clippy::disallowed_methods,
        reason = "native writable lock capture only; Windows rooted writers refuse"
    )]
    let database = lbug::Database::new(path.join("rows.lbdb"), super::tests::config());
    #[cfg(not(windows))]
    let database = super::open::open(
        &lbug::RootDirectory::open(path)?,
        "rows.lbdb",
        super::tests::config(),
    );
    database
}

#[test]
fn graph_probe_captures_native_writable_contention_only_in_test_helper() {
    let fixture = Fixture::new();
    drop(fixture.writer());
    let holder = Holder::new(&fixture, "native");
    let error = native_writer(&fixture.path).unwrap_err().to_string();
    println!("captured native OS diagnostic: {error}");
    let target = if cfg!(windows) {
        fixture.path.join("rows.lbdb").display().to_string()
    } else {
        "rows.lbdb".to_owned()
    };
    let classified = super::probe::classify_native(&error, &target, NativePlatform::current());
    assert!(
        matches!(
            classified,
            ProbeError::Locked(_) | ProbeError::Unreadable(_)
        ),
        "{classified:?}"
    );
    if cfg!(target_os = "linux") {
        assert!(
            matches!(classified, ProbeError::Locked(_)),
            "observed Linux PID form must classify: {classified:?}"
        );
    }
    drop(holder);
}

#[test]
fn graph_probe_only_observed_windows_lock_structure_and_actual_target_are_locked() {
    let captured = "IO exception: Could not set lock on file : clock.lbdb \
        (Error: 33)\n\
        See the docs: https://docs.ladybugdb.com/concurrency for more information.";
    assert_eq!(
        super::probe::classify_native(captured, "clock.lbdb", NativePlatform::Windows),
        ProbeError::Locked(captured.into())
    );
    for (message, target, windows) in [
        (captured, "clock.lbdb", NativePlatform::Other),
        (captured, "other.lbdb", NativePlatform::Windows),
        (
            "Could not set lock on file",
            "clock.lbdb",
            NativePlatform::Windows,
        ),
        (
            "Cannot open file. path: clock.lbdb - Error 33: lock",
            "clock.lbdb",
            NativePlatform::Windows,
        ),
        (
            "block clock deadlock",
            "clock.lbdb",
            NativePlatform::Windows,
        ),
    ] {
        assert_eq!(
            super::probe::classify_native(message, target, windows),
            ProbeError::Unreadable(message.into())
        );
    }
    for message in [
        captured.replace("33", "5"),
        captured.replace("33", "133"),
        format!("{captured}extra"),
        captured.replace("clock.lbdb", "clock.lbdb-wal"),
        captured.replace(" (Error: 33)", " (Lock is held by PID 123)"),
    ] {
        assert_eq!(
            super::probe::classify_native(&message, "clock.lbdb", NativePlatform::Windows),
            ProbeError::Unreadable(message)
        );
    }
}

#[test]
fn graph_probe_explicit_settings_validate_all_approved_boundaries() {
    use crate::graph::projection::EngineSettings;
    use maestro_kernel::artifact::Digest;
    let min = 16 * 1024 * 1024;
    let settings =
        |pool, size, threads| EngineSettings::new(pool, size, threads, Digest::of(b"frozen-lock"));
    let base = settings(min, 64 * 1024 * 1024, 1).unwrap();
    assert_eq!(
        format!("{:?}", super::config::native(&base).read_only(true)),
        format!("{:?}", super::tests::config().read_only(true))
    );
    for value in [min, 1024 * 1024 * 1024] {
        assert!(settings(value, base.max_db_size, base.max_num_threads).is_ok());
    }
    for value in [min, 1024 * 1024 * 1024 * 1024] {
        assert!(settings(base.buffer_pool_size, value, base.max_num_threads).is_ok());
    }
    for value in [1, 64] {
        assert!(settings(base.buffer_pool_size, base.max_db_size, value).is_ok());
    }
    for value in [0, min - 1, 1024 * 1024 * 1024 + 1] {
        assert!(settings(value, base.max_db_size, base.max_num_threads).is_err());
    }
    for value in [
        0,
        8 * 1024 * 1024,
        24 * 1024 * 1024,
        2 * 1024 * 1024 * 1024 * 1024,
    ] {
        assert!(settings(base.buffer_pool_size, value, base.max_num_threads).is_err());
    }
    for value in [0, 65] {
        assert!(settings(base.buffer_pool_size, base.max_db_size, value).is_err());
    }
}

#[test]
fn graph_probe_linux_capture_requires_canonical_positive_pid_and_exact_target_suffix() {
    let captured = "IO exception: Could not set lock on file : clock.lbdb \
        (Lock is held by PID 3441212)\n\
        See the docs: https://docs.ladybugdb.com/concurrency for more information.";
    assert_eq!(
        super::probe::classify_native(captured, "clock.lbdb", NativePlatform::Linux),
        ProbeError::Locked(captured.into())
    );
    assert_eq!(
        super::probe::classify_native(captured, "clock.lbdb", NativePlatform::Other),
        ProbeError::Unreadable(captured.into())
    );
    assert_eq!(
        super::probe::classify_native(captured, "other.lbdb", NativePlatform::Linux),
        ProbeError::Unreadable(captured.into())
    );
    for message in [
        captured.replace("3441212", "0"),
        captured.replace("3441212", "abc"),
        captured.replace("3441212", "-1"),
        captured.replace("3441212", "+1"),
        captured.replace("3441212", "01"),
        captured.replace("3441212", "4294967296"),
        captured.replace("more information.", "more info."),
        format!("{captured}extra"),
        captured.replace(
            "\nSee the docs: https://docs.ladybugdb.com/concurrency for more information.",
            "",
        ),
    ] {
        assert_eq!(
            super::probe::classify_native(&message, "clock.lbdb", NativePlatform::Linux),
            ProbeError::Unreadable(message)
        );
    }
}
