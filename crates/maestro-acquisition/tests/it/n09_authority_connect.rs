//! Supervisor-approved N05 fix: IPC connect never blocks on a full backlog.
#![cfg(target_os = "linux")]
use maestro_acquisition::{Refusal, policy::authority_socket::connect_bounded};
use maestro_kernel::retrieval::Clock;
use maestro_test_scratch::scratch_directory;
use rustix::net::listen;
use std::{
    fs,
    os::unix::net::{UnixListener, UnixStream},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

/// A host observation at the cutoff; no wall-clock waiting or sleeps.
#[derive(Debug)]
struct FixedClock(Instant);
impl Clock for FixedClock {
    fn now(&self) -> Instant {
        self.0
    }
}
#[test]
fn n09_authority_full_backlog_refuses_without_waiting() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("authority.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listen(&listener, 0).unwrap();
    let _queued = UnixStream::connect(&path).unwrap();
    let start = Instant::now();
    let deadline = start + Duration::from_secs(1);
    let clock = AdvancingClock {
        start,
        calls: AtomicUsize::new(0),
        after: 1,
    };
    assert_eq!(
        connect_bounded(&path, deadline, &clock).unwrap_err(),
        Refusal::Unqualified
    );
    fs::remove_dir_all(scratch).unwrap();
}
#[test]
fn n09_authority_normal_connect_neighbour_succeeds() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("authority.sock");
    let _listener = UnixListener::bind(&path).unwrap();
    let now = Instant::now();
    assert!(connect_bounded(&path, now + Duration::from_secs(1), &FixedClock(now)).is_ok());
    fs::remove_dir_all(scratch).unwrap();
}

/// The second observation reaches the cutoff, after the nonblocking syscall.
#[derive(Debug)]
struct AdvancingClock {
    start: Instant,
    calls: AtomicUsize,
    /// Number of observations before the cutoff.
    after: usize,
}
impl Clock for AdvancingClock {
    fn now(&self) -> Instant {
        if self.calls.fetch_add(1, Ordering::SeqCst) < self.after {
            self.start
        } else {
            self.start + Duration::from_secs(1)
        }
    }
}
#[test]
fn n09_authority_completed_connect_at_deadline_refuses() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("authority.sock");
    let _listener = UnixListener::bind(&path).unwrap();
    let start = Instant::now();
    let clock = AdvancingClock {
        start,
        calls: AtomicUsize::new(0),
        after: 1,
    };
    assert_eq!(
        connect_bounded(&path, start + Duration::from_secs(1), &clock).unwrap_err(),
        Refusal::Deadline
    );
    fs::remove_dir_all(scratch).unwrap();
}
#[test]
fn n09_authority_unconnected_backlog_socket_cannot_grant_success() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("authority.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listen(&listener, 0).unwrap();
    let _queued = UnixStream::connect(&path).unwrap();
    let now = Instant::now();
    let clock = AdvancingClock {
        start: now,
        calls: AtomicUsize::new(0),
        after: 2,
    };
    assert_eq!(
        connect_bounded(&path, now + Duration::from_secs(1), &clock).unwrap_err(),
        Refusal::Unqualified
    );
    fs::remove_dir_all(scratch).unwrap();
}
