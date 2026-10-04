//! Linux IPC policy contracts; kernel peer identity is never supplied by JSON.
#![cfg(target_os = "linux")]
use maestro_acquisition::{
    Refusal,
    policy::{
        authority::{Authority, AuthorityRefusal, Operation, Target},
        authority_socket::{AuthoritySocket, connect_bounded, read_frame},
    },
};
use maestro_kernel::retrieval::{Clock, SystemClock};
use maestro_test_scratch::scratch_directory;
use rustix::{
    fs::{OFlags, fcntl_getfl},
    net::listen,
    process::geteuid,
};
use std::{
    fs,
    io::{BufRead as _, BufReader, Write as _},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};

/// Exact synthetic resource; grants are not involved in guard tests.
fn target() -> Target {
    Target {
        scope: "workspace/default/collection/garden".into(),
        source: "notes".into(),
        account: "public".into(),
        resource: "https://garden.example/docs".into(),
    }
}

#[test]
fn policy_socket_independent_identity_guards() {
    let uid = geteuid().as_raw();
    for (authority_uid, principal) in [(uid, uid.to_string()), (uid + 1, "not-the-uid".into())] {
        let socket = AuthoritySocket {
            socket: "missing.sock".into(),
            authority_uid,
        };
        assert_eq!(
            socket.decide(&principal, Operation::Fetch, &target(), UNIX_EPOCH),
            Err(AuthorityRefusal::Refused(Refusal::Access))
        );
    }
}

#[test]
fn policy_socket_kernel_peer_mismatch_precedes_response() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("authority.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = thread::spawn(move || {
        use rustix::event::{PollFd, PollFlags, Timespec, poll};
        let mut fd = [PollFd::new(&listener, PollFlags::IN)];
        if poll(
            &mut fd,
            Some(&Timespec::try_from(Duration::from_secs(1)).unwrap()),
        )
        .unwrap()
            == 0
        {
            return;
        }
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let mut request = String::new();
        drop(BufReader::new(&mut peer).read_line(&mut request));
        // A forged positive JSON response cannot authenticate this same-UID server.
        drop(peer.write_all(b"{\"status\":\"permit\",\"grant_id\":\"forged\"}\n"));
    });
    let uid = geteuid().as_raw();
    let socket = AuthoritySocket {
        socket: path,
        authority_uid: uid + 1,
    };
    let result = socket.decide(&uid.to_string(), Operation::Fetch, &target(), UNIX_EPOCH);
    server.join().unwrap();
    fs::remove_dir_all(scratch).unwrap();
    assert_eq!(result, Err(AuthorityRefusal::Refused(Refusal::Unqualified)));
}

#[test]
fn policy_socket_complete_and_invalid_frames() {
    for (input, accepted) in [
        (b"{}\n".as_slice(), true),
        (b"{}\nx".as_slice(), false),
        (b"".as_slice(), false),
    ] {
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(input).unwrap();
        writer.shutdown(Shutdown::Write).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| read_frame(&mut reader)));
        assert!(result.is_ok(), "frame decoder panicked");
        let result = result.unwrap();
        if accepted {
            assert_eq!(result, Ok(input.to_vec()));
        } else {
            assert_eq!(result, Err(Refusal::Unqualified));
        }
    }
    for size in [65_536, 65_537, 70_000] {
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        let mut input = vec![b'a'; size];
        *input.last_mut().unwrap() = b'\n';
        let sender = thread::spawn(move || {
            drop(writer.write_all(&input));
        });
        let result = catch_unwind(AssertUnwindSafe(|| read_frame(&mut reader)));
        drop(reader);
        sender.join().unwrap();
        assert!(result.is_ok(), "frame decoder panicked");
        let result = result.unwrap();
        if size == 65_536 {
            let mut expected = vec![b'a'; size];
            *expected.last_mut().unwrap() = b'\n';
            assert_eq!(result, Ok(expected));
        } else {
            assert_eq!(result, Err(Refusal::Unqualified));
        }
    }
}

/// Clock moves to the cutoff after the first connect syscall; no real sleep.
#[derive(Debug)]
struct CutoffClock {
    start: Instant,
    calls: AtomicUsize,
}
impl Clock for CutoffClock {
    fn now(&self) -> Instant {
        self.start
            + if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Duration::ZERO
            } else {
                Duration::from_secs(1)
            }
    }
}

#[test]
fn policy_socket_full_backlog_never_blocks() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("authority.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listen(&listener, 0).unwrap();
    let _queued = UnixStream::connect(&path).unwrap();
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let start = Instant::now();
        let clock = CutoffClock {
            start,
            calls: AtomicUsize::new(0),
        };
        let result = connect_bounded(&path, start + Duration::from_secs(1), &clock).map(|_| ());
        sender.send(result).unwrap();
    });
    let result = receiver.recv_timeout(Duration::from_secs(10));
    if result.is_err() {
        // Release an accidentally blocking connect before joining the test worker.
        drop(listener.accept().unwrap());
    }
    worker.join().unwrap();
    fs::remove_dir_all(scratch).unwrap();
    assert!(result.is_ok(), "nonblocking connect exceeded test deadline");
    assert_eq!(result.unwrap(), Err(Refusal::Unqualified));
}

#[test]
fn policy_socket_failed_connect_preserves_error_before_deadline() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("missing.sock");
    let start = Instant::now();
    let clock = CutoffClock {
        start,
        calls: AtomicUsize::new(0),
    };
    let result = connect_bounded(&path, start + Duration::from_secs(1), &clock).map(|_| ());
    fs::remove_dir_all(scratch).unwrap();
    assert_eq!(result, Err(Refusal::Unqualified));
}

#[test]
fn policy_socket_expired_deadline_precedes_connect() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("missing.sock");
    let start = Instant::now();
    let clock = CutoffClock {
        start,
        calls: AtomicUsize::new(0),
    };
    let result = connect_bounded(&path, start, &clock).map(|_| ());
    fs::remove_dir_all(scratch).unwrap();
    assert_eq!(result, Err(Refusal::Deadline));
}

#[test]
fn policy_socket_connected_stream_is_blocking() {
    let scratch = scratch_directory().unwrap();
    let path = scratch.join("authority.sock");
    let _listener = UnixListener::bind(&path).unwrap();
    let start = Instant::now();
    // The real clock is appropriate for an immediately available local connection.
    let socket = connect_bounded(&path, start + Duration::from_secs(1), &SystemClock).unwrap();
    assert!(!fcntl_getfl(&socket).unwrap().contains(OFlags::NONBLOCK));
    fs::remove_dir_all(scratch).unwrap();
}
