//! Actual Unix IPC framing refuses oversized, partial and cumulative-timeout input.
#![cfg(target_os = "linux")]
use super::support::Home;
use maestro_acquisition::{
    Refusal,
    policy::{
        authority::{Authority as _, AuthorityRefusal, Operation, Target},
        authority_socket::{AuthoritySocket, frame_timeout, read_frame, read_frame_with_clock},
    },
};
use maestro_kernel::retrieval::Clock;
use std::{
    fs, future,
    io::{BufRead as _, BufReader, Write as _},
    os::unix::{
        fs::PermissionsExt as _,
        net::{UnixListener, UnixStream},
    },
    path::Path,
    process::Command,
    str,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime},
};
use tokio::time::Instant as TokioInstant;

#[test]
fn n05_ipc_frames_bound_bytes_termination_and_cumulative_deadline() {
    for input in [
        b"{}\n".to_vec(),
        {
            let mut bytes = vec![b' '; 65_537];
            bytes.extend_from_slice(b"{}\n");
            bytes
        },
        b"unterminated".to_vec(),
        b"{}\njunk".to_vec(),
    ] {
        let allowed = input == b"{}\n";
        let (mut sender, mut receiver) = UnixStream::pair().unwrap();
        let writer = thread::spawn(move || {
            drop(sender.write_all(&input));
        });
        let expected = if allowed {
            Ok(b"{}\n".to_vec())
        } else {
            Err(Refusal::Unqualified)
        };
        assert_eq!(read_frame(&mut receiver), expected);
        drop(receiver);
        writer.join().unwrap();
    }
}

#[test]
fn n05_read_only_client_refuses_same_user_and_wrong_authority_peer() {
    let home = Home::bare();
    let socket = home.tools().join("authority.sock");
    let server = fake_authority(
        &socket,
        b"{\"status\":\"permit\",\"grant_id\":\"forged\"}\n",
    );
    let uid = Command::new("id").arg("-u").output().unwrap();
    let principal = str::from_utf8(&uid.stdout).unwrap().trim();
    let uid: u32 = principal.parse().unwrap();
    let target = Target {
        scope: "workspace/default/collection/synthetic".into(),
        source: "handbook".into(),
        account: "public".into(),
        resource: "https://synthetic.example/manual".into(),
    };
    let same = AuthoritySocket {
        socket: socket.clone(),
        authority_uid: uid,
    };
    assert_eq!(
        same.decide(principal, Operation::Fetch, &target, SystemTime::now()),
        Err(AuthorityRefusal::Refused(Refusal::Access))
    );
    let wrong = AuthoritySocket {
        socket,
        authority_uid: uid + 1,
    };
    assert_eq!(
        wrong.decide(principal, Operation::Fetch, &target, SystemTime::now()),
        Err(AuthorityRefusal::Refused(Refusal::Unqualified))
    );
    assert_eq!(
        wrong.decide(
            "model-approved",
            Operation::Fetch,
            &target,
            SystemTime::now()
        ),
        Err(AuthorityRefusal::Refused(Refusal::Access))
    );
    server.join().unwrap();
}

/// An independently implemented hostile local endpoint; never qualifies a host or writes grants.
fn fake_authority(socket: &Path, response: &'static [u8]) -> thread::JoinHandle<()> {
    let listener = UnixListener::bind(socket).unwrap();
    thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        let mut request = String::new();
        drop(BufReader::new(&peer).read_line(&mut request));
        drop(peer.write_all(response));
    })
}

#[test]
fn n05_owner_command_refuses_wrong_peer_refusals_and_content_bearing_replies() {
    let uid = Command::new("id").arg("-u").output().unwrap();
    let uid: u32 = str::from_utf8(&uid.stdout).unwrap().trim().parse().unwrap();
    for (expected_uid, response) in [
        (uid + 1, b"{\"status\":\"ok\"}\n".as_slice()),
        (uid, b"{\"status\":\"refused\"}\n".as_slice()),
        (
            uid,
            b"{\"status\":\"ok\",\"private_content\":\"synthetic-canary\"}\n".as_slice(),
        ),
    ] {
        let home = Home::bare();
        let socket = home.tools().join("authority.sock");
        let server = fake_authority(&socket, response);
        let grant = serde_json::json!({
            "id":"synthetic",
            "principal":"65534",
            "operation":"fetch",
            "target":{"scope":"workspace/default/collection/synthetic",
            "source":"handbook",
            "account":"public",
            "resource":"https://synthetic.example/manual"},
            "expires_at":"2099-01-01T00:00:00Z"});
        let file = home.root().join("grant.json");
        fs::write(
            &file,
            serde_json::to_vec(
                &serde_json::json!({"action":"grant","grant":grant,"confirmation":grant}),
            )
            .unwrap(),
        )
        .unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        let ended = home.run(&[
            "authority",
            "request",
            "--socket",
            socket.to_str().unwrap(),
            "--authority-uid",
            &expected_uid.to_string(),
            "--file",
            file.to_str().unwrap(),
        ]);
        assert_eq!(ended.code, Some(2), "{ended:?}");
        assert!(!ended.stdout.contains("synthetic-canary"));
        assert!(!ended.stderr.contains("synthetic-canary"));
        server.join().unwrap();
    }
}

/// Deterministic monotonic budget; no sleep, timer or wall-clock deadline drives it.
#[derive(Debug)]
struct SteppedClock {
    start: Instant,
    calls: AtomicU64,
}
impl Clock for SteppedClock {
    fn now(&self) -> Instant {
        self.start + Duration::from_millis(750 * self.calls.fetch_add(1, Ordering::SeqCst))
    }
}

#[tokio::test]
async fn n05_partial_frames_exhaust_the_stopped_clock_cumulative_budget() {
    let (mut sender, mut receiver) = UnixStream::pair().unwrap();
    let mut bytes = vec![b' '; 8192];
    bytes.extend_from_slice(b"{}\n");
    sender.write_all(&bytes).unwrap();
    maestro_test_clock::on_stopped_clock(future::pending(), || async {
        let clock = SteppedClock {
            start: TokioInstant::now().into_std(),
            calls: AtomicU64::new(0),
        };
        assert_eq!(
            read_frame_with_clock(&mut receiver, &clock),
            Err(Refusal::Deadline)
        );
        assert_eq!(clock.calls.load(Ordering::SeqCst), 4);
    })
    .await;
}

#[test]
fn n05_os_read_timeouts_are_positive_remaining_budgets_without_resets() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(2);
    assert_eq!(frame_timeout(deadline, now), Ok(Duration::from_secs(2)));
    assert_eq!(
        frame_timeout(deadline, now + Duration::from_millis(750)),
        Ok(Duration::from_millis(1250))
    );
    for reached in [deadline, deadline + Duration::from_nanos(1)] {
        assert_eq!(frame_timeout(deadline, reached), Err(Refusal::Deadline));
    }
}
