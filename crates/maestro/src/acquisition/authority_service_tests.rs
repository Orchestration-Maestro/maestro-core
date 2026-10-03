//! Synthetic IPC peers exercise the real authentication and mutation route.
use super::{
    authority_host::Host,
    authority_service::{Request, SocketPath, change, handle, request},
    authority_store::Store,
};
use crate::failure::Failure;
use maestro_acquisition::policy::authority::{Grant, Operation, Target};
use maestro_test_scratch::scratch_directory;
use rustix::process::geteuid;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead as _, BufReader, Write as _},
    os::unix::{
        fs::PermissionsExt as _,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::mpsc,
    thread,
    time::Duration,
};

/// Owned synthetic authority directory; no host qualification is asserted.
struct Fixture {
    root: PathBuf,
    host: Host,
    store: Store,
    grant: Grant,
}
impl Fixture {
    fn new() -> Self {
        let root = scratch_directory().unwrap();
        let uid = geteuid().as_raw();
        let host = Host {
            store: root.clone(),
            socket: root.join("socket"),
            owner_uid: uid,
            pipeline_uid: uid + 1,
            connector_uid: uid + 2,
            launcher: root.join("unused"),
        };
        let store = Store::open(&host).unwrap();
        let grant = Grant {
            id: "synthetic-grant".into(),
            principal: host.pipeline_uid.to_string(),
            operation: Operation::Fetch,
            expires_at: "2099-01-01T00:00:00Z".into(),
            target: Target {
                scope: "workspace/default/collection/synthetic".into(),
                source: "handbook".into(),
                account: "public".into(),
                resource: "https://synthetic.example:443/manual".into(),
            },
        };
        Self {
            root,
            host,
            store,
            grant,
        }
    }
    fn finish(self) {
        drop(self.store);
        fs::remove_dir_all(self.root).unwrap();
    }
}

#[test]
fn debt_service_changes_require_owner_confirmation_and_either_pipeline() {
    let mut fixture = Fixture::new();
    let owner = fixture.host.owner_uid;
    let grant = fixture.grant.clone();
    for principal in [fixture.host.pipeline_uid, fixture.host.connector_uid] {
        let mut grant = grant.clone();
        grant.principal = principal.to_string();
        assert_eq!(
            change(
                &fixture.host,
                &mut fixture.store,
                owner,
                (&grant, &grant),
                false
            )
            .unwrap(),
            json!({"status":"ok"})
        );
        assert_eq!(
            change(
                &fixture.host,
                &mut fixture.store,
                owner,
                (&grant, &grant),
                true
            )
            .unwrap(),
            json!({"status":"ok"})
        );
    }
    let mut mismatch = grant.clone();
    mismatch.expires_at = "2098-01-01T00:00:00Z".into();
    assert!(
        change(
            &fixture.host,
            &mut fixture.store,
            owner,
            (&grant, &mismatch),
            false
        )
        .is_err()
    );
    assert!(
        change(
            &fixture.host,
            &mut fixture.store,
            owner + 1,
            (&grant, &grant),
            false
        )
        .is_err()
    );
    let mut outsider = grant.clone();
    outsider.principal = (owner + 3).to_string();
    assert!(
        change(
            &fixture.host,
            &mut fixture.store,
            owner,
            (&outsider, &outsider),
            false
        )
        .is_err()
    );
    fixture.finish();
}

#[test]
fn debt_service_decisions_allow_owner_or_exact_kernel_peer() {
    let mut fixture = Fixture::new();
    let uid = geteuid().as_raw();
    let mut grant = fixture.grant.clone();
    grant.principal = uid.to_string();
    grant.id = "self-grant".into();
    fixture.store.change(&grant, false, uid).unwrap();
    fixture.store.change(&fixture.grant, false, uid).unwrap();
    for (owner, principal, accepted) in [
        (uid, fixture.grant.principal.clone(), true),
        (uid + 10, uid.to_string(), true),
        (uid + 10, fixture.grant.principal.clone(), false),
    ] {
        fixture.host.owner_uid = owner;
        let (mut peer, mut server) = UnixStream::pair().unwrap();
        let value = Request::Decide {
            principal,
            operation: grant.operation,
            target: grant.target.clone(),
        };
        writeln!(peer, "{}", serde_json::to_string(&value).unwrap()).unwrap();
        let response = handle(&mut server, &fixture.host, &mut fixture.store);
        if accepted {
            assert_eq!(response.unwrap()["status"], "permit");
        } else {
            assert!(response.is_err());
        }
    }
    fixture.finish();
}

/// Reply through an actual same-UID Unix socket, after receiving the complete request.
fn roundtrip(fixture: &Fixture, value: &Request, response: Value) -> Result<Value, Failure> {
    let file = fixture.root.join("request");
    fs::write(&file, serde_json::to_vec(value).unwrap()).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let listener = UnixListener::bind(&fixture.host.socket).unwrap();
    let guard = SocketPath(fixture.host.socket.clone());
    let (completed, observed) = mpsc::channel();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert!(!line.is_empty());
        writeln!(stream, "{response}").unwrap();
        completed.send(()).unwrap();
    });
    let result = request(&fixture.host.socket, fixture.host.owner_uid, &file);
    observed
        .recv_timeout(Duration::from_secs(10))
        .expect("owner request did not reach the authenticated server");
    worker.join().unwrap();
    drop(guard);
    result
}

#[test]
fn debt_service_owner_requests_require_exact_success_shape() {
    let fixture = Fixture::new();
    let grant = fixture.grant.clone();
    let decide = Request::Decide {
        principal: grant.principal.clone(),
        operation: grant.operation,
        target: grant.target.clone(),
    };
    assert_eq!(
        roundtrip(
            &fixture,
            &decide,
            json!({"status":"permit","grant_id":"opaque"})
        )
        .unwrap(),
        json!({"status":"permit","grant_id":"opaque"})
    );
    for value in [
        Request::Grant {
            grant: grant.clone(),
            confirmation: grant.clone(),
        },
        Request::Revoke {
            grant: grant.clone(),
            confirmation: grant.clone(),
        },
    ] {
        assert_eq!(
            roundtrip(&fixture, &value, json!({"status":"ok"})).unwrap(),
            json!({"status":"ok"})
        );
    }
    let expired = roundtrip(
        &fixture,
        &decide,
        json!({"status":"expired","grant_id":"opaque"}),
    );
    assert!(matches!(expired, Err(Failure::Refused(message)) if message == "authority refused"));
    fixture.finish();
}

#[test]
fn debt_service_socket_path_drop_unlinks_owned_endpoint() {
    let root = scratch_directory().unwrap();
    let path = root.join("socket");
    let listener = UnixListener::bind(&path).unwrap();
    drop(SocketPath(path.clone()));
    assert!(!path.exists());
    drop(listener);
    fs::remove_dir_all(root).unwrap();
}
