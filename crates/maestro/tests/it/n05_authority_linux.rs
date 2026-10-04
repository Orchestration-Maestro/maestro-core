//! Qualified Linux authority fixtures; only the launcher uses sudo.
#![cfg(target_os = "linux")]
use super::support::{Home, Running};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    os::unix::fs::PermissionsExt,
    os::unix::net::UnixStream,
    path::Path,
    process::{Command, Output},
    str, thread,
    time::{Duration, Instant},
};

/// A disk scratch authority and its separate pipeline/connector identities.
pub(super) struct Fixture {
    pub(super) home: Home,
    pub(super) config: String,
    pub(super) store: String,
    pub(super) socket: String,
    pub(super) owner_uid: String,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let home = Home::bare();
        let base = home.tools().join("authority");
        fs::create_dir(&base).unwrap();
        fs::set_permissions(home.tools(), fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o755)).unwrap();
        let store = base.join("store");
        fs::create_dir(&store).unwrap();
        fs::set_permissions(&store, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = base.join("authority.sock");
        let launcher = base.join("launcher");
        fs::write(
            &launcher,
            "#!/bin/sh\n\
            uid=$1\n\
            shift\n\
            case $uid in 65534|65533) ;; *) exit 1;; esac\n\
            exec sudo -n setpriv --reuid=\"$uid\" --regid=\"$uid\" \
            --clear-groups -- \"$@\"\n",
        )
        .unwrap();
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
        // Copy the independently launched probe binary under traversable scratch.
        fs::copy(env!("CARGO_BIN_EXE_maestro"), base.join("maestro")).unwrap();
        fs::set_permissions(base.join("maestro"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::copy(env::current_exe().unwrap(), base.join("peer-fixture")).unwrap();
        fs::set_permissions(base.join("peer-fixture"), fs::Permissions::from_mode(0o755)).unwrap();
        let config = base.join("config.json");
        let uid = Command::new("id").arg("-u").output().unwrap();
        fs::write(
            &config,
            serde_json::to_vec(&json!({
            "store":store,
            "socket":socket,
            "owner_uid":str::from_utf8(&uid.stdout).unwrap().trim().parse::<u32>().unwrap(),
            "pipeline_uid":65534,
            "connector_uid":65533,
            "launcher":launcher}))
            .unwrap(),
        )
        .unwrap();
        Self {
            home,
            config: config.to_str().unwrap().into(),
            store: store.to_str().unwrap().into(),
            socket: socket.to_str().unwrap().into(),
            owner_uid: str::from_utf8(&uid.stdout).unwrap().trim().into(),
        }
    }
    pub(super) fn qualify(&self) {
        let ended = self
            .home
            .run(&["authority", "qualify", "--config", &self.config]);
        assert_eq!(ended.code, Some(0), "{ended:?}");
    }
    pub(super) fn serve(&self) -> Running {
        let started = Instant::now();
        let mut running = self
            .home
            .start(&["authority", "serve", "--config", &self.config]);
        // Event-driven ready signal; Running's 15s deadline is only a hang guard.
        running.line_with("authority ready");
        assert!(Path::new(&self.socket).exists());
        eprintln!("n05 serve-bind {:.3}s", started.elapsed().as_secs_f64());
        running
    }
    fn as_uid(&self, uid: &str, value: &Value) -> Output {
        let path = self.home.tools().join("authority/request.json");
        fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        Command::new("sudo")
            .args([
                "-n",
                "setpriv",
                "--reuid",
                uid,
                "--regid",
                uid,
                "--clear-groups",
                "--",
            ])
            .arg(self.home.tools().join("authority/maestro"))
            .args([
                "authority",
                "request",
                "--socket",
                &self.socket,
                "--authority-uid",
                &self.owner_uid,
                "--file",
                path.to_str().unwrap(),
            ])
            .output()
            .unwrap()
    }
    fn hostile_peer(&self, uid: &str, value: &Value) -> Value {
        let path = self.home.tools().join("authority/peer-request.json");
        fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        let output = Command::new("sudo")
            .args([
                "-n",
                "--preserve-env=MAESTRO_N05_SOCKET,MAESTRO_N05_REQUEST",
                "setpriv",
                "--reuid",
                uid,
                "--regid",
                uid,
                "--clear-groups",
                "--",
            ])
            .arg(self.home.tools().join("authority/peer-fixture"))
            .args([
                "--exact",
                "n05_authority_linux::n05_unprivileged_peer_uses_independent_socket_requests",
                "--ignored",
                "--nocapture",
            ])
            .env("MAESTRO_N05_SOCKET", &self.socket)
            .env("MAESTRO_N05_REQUEST", path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let text = str::from_utf8(&output.stdout).unwrap();
        let reply = text
            .lines()
            .find_map(|line| line.strip_prefix("authority peer reply: "))
            .unwrap();
        serde_json::from_str(reply).unwrap()
    }
    pub(super) fn request(&self, value: &Value) -> Value {
        let mut socket = UnixStream::connect(&self.socket).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        writeln!(socket, "{value}").unwrap();
        let mut response = String::new();
        BufReader::new(socket).read_line(&mut response).unwrap();
        serde_json::from_str(&response).unwrap()
    }
}
/// Exact synthetic grant, not a manifest approval.
pub(super) fn grant() -> Value {
    json!({
        "id":"fixture-grant",
        "principal":"65534",
        "operation":"fetch",
        "target":{"scope":"workspace/default/collection/synthetic",
        "source":"handbook",
        "account":"synthetic-account",
        "resource":"https://synthetic.example/manual"},
        "expires_at":"2099-01-01T00:00:00Z"})
}

/// Malformed approvals and incomplete exact confirmations have no effects.
fn refuse_forged(fixture: &Fixture, grant: &Value) {
    // Unauthenticated JSON claims and model approvals cannot create authority.
    for forged in [
        json!({"action":"grant","grant":grant,"confirmation":"yes"}),
        json!({"action":"grant","grant":grant,"approval":"model-approved"}),
        json!({"action":"grant","grant":grant,"confirmation":grant,"approval":"model-approved"}),
    ] {
        assert_eq!(fixture.request(&forged)["status"], "refused");
    }
    for field in ["scope", "source", "account", "resource", "expires_at"] {
        let mut confirmation = grant.clone();
        if field == "expires_at" {
            confirmation[field] = json!("2098-01-01T00:00:00Z");
        } else {
            confirmation["target"][field] = json!("other");
        }
        assert_eq!(
            fixture.request(&json!({
                "action":"grant",
                "grant":grant,
                "confirmation":confirmation}))["status"],
            "refused"
        );
    }
    let mut unassigned = grant.clone();
    unassigned["principal"] = json!(fixture.owner_uid);
    assert_eq!(
        fixture.request(&json!({
            "action":"grant",
            "grant":unassigned,
            "confirmation":unassigned}))["status"],
        "refused"
    );
}

/// Actual platform peers, not logical identity labels, enforce the writer boundary.
fn refuse_mutations(fixture: &Fixture, grant: &Value, edited: &Value) {
    // Real unprivileged peers cannot grant, edit or revoke, even with exact confirmation.
    for uid in ["65534", "65533"] {
        for value in [
            json!({"action":"grant","grant":grant,"confirmation":grant}),
            json!({"action":"grant","grant":grant,"confirmation":grant}),
            json!({"action":"revoke","grant":edited,"confirmation":edited}),
        ] {
            let ended = fixture.as_uid(uid, &value);
            assert_eq!(ended.status.code(), Some(2), "{ended:?}");
            assert!(
                String::from_utf8_lossy(&ended.stderr).contains("authority refused"),
                "{ended:?}"
            );
        }
    }
}

#[test]
#[ignore = "needs sudo: real pipeline/connector UID separation in ephemeral scratch"]
fn n05_needs_sudo_owner_grants_audit_expiry_revocation_and_unprivileged_mutations() {
    let fixture = Fixture::new();
    fixture.qualify();
    let service = fixture.serve();
    let grant = grant();
    refuse_forged(&fixture, &grant);
    let request = json!({"action":"grant","grant":grant,"confirmation":grant});
    assert_eq!(fixture.request(&request)["status"], "ok");
    let mut edited = grant.clone();
    edited["expires_at"] = json!("2098-01-01T00:00:00Z");
    assert_eq!(
        fixture.request(&json!({"action":"grant","grant":edited,"confirmation":edited}))["status"],
        "ok"
    );
    refuse_mutations(&fixture, &grant, &edited);
    let decide =
        json!({"action":"decide","principal":"65534","operation":"fetch","target":grant["target"]});
    let granted = fixture.as_uid("65534", &decide);
    assert_eq!(granted.status.code(), Some(0), "{granted:?}");
    assert_eq!(fixture.hostile_peer("65533", &decide)["status"], "refused");
    let denied = fixture.as_uid("65533", &decide);
    assert_eq!(denied.status.code(), Some(2), "{denied:?}");
    // Owner may inspect exact read-only decisions, with no grant mutation.
    assert_eq!(fixture.request(&decide)["status"], "permit");
    let mut wrong = decide.clone();
    wrong["target"]["account"] = json!("other-account");
    assert_eq!(fixture.request(&wrong)["status"], "refused");
    assert_eq!(
        fixture.request(&json!({"action":"revoke","grant":grant,"confirmation":grant}))["status"],
        "refused",
        "stale full confirmation must not revoke an edited grant"
    );
    assert_eq!(
        fixture.request(&json!({"action":"revoke","grant":edited,"confirmation":edited}))["status"],
        "ok"
    );
    assert_eq!(fixture.request(&decide)["status"], "refused");
    let mut expired = grant.clone();
    expired["expires_at"] = json!("2000-01-01T00:00:00Z");
    assert_eq!(
        fixture.request(&json!({
            "action":"grant",
            "grant":expired,
            "confirmation":expired}))["status"],
        "refused"
    );
    assert_audit(&fixture, &grant, &edited);
    drop(service);
}

/// Successful owner changes, and only those changes, retain exact immutable audit records.
fn assert_audit(fixture: &Fixture, grant: &Value, edited: &Value) {
    let database =
        rusqlite::Connection::open(Path::new(&fixture.store).join("authority.sqlite3")).unwrap();
    let count: u32 = database
        .query_row("SELECT count(*) FROM audit", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 4);
    let audit: Vec<(u32, String, String)> = database
        .prepare("SELECT peer_uid,action,record FROM audit ORDER BY sequence")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        audit
            .iter()
            .all(|(uid, _, _)| uid.to_string() == fixture.owner_uid)
    );
    assert_eq!(
        audit
            .iter()
            .map(|(_, action, _)| action.as_str())
            .collect::<Vec<_>>(),
        ["grant", "grant", "revoke", "expired_refusal"]
    );
    assert_eq!(serde_json::from_str::<Value>(&audit[0].2).unwrap(), *grant);
    assert_eq!(serde_json::from_str::<Value>(&audit[1].2).unwrap(), *edited);
}

#[test]
fn n05_stale_receipt_and_same_user_setup_refuse_live_grants() {
    let fixture = Fixture::new();
    let missing = fixture
        .home
        .run(&["authority", "serve", "--config", &fixture.config]);
    assert_eq!(missing.code, Some(2), "{missing:?}");
    assert!(!Path::new(&fixture.store).join("authority.sqlite3").exists());
    fs::write(
        Path::new(&fixture.store).join("qualification"),
        "forged-receipt",
    )
    .unwrap();
    fs::set_permissions(
        Path::new(&fixture.store).join("qualification"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let ended = fixture
        .home
        .run(&["authority", "serve", "--config", &fixture.config]);
    assert_eq!(ended.code, Some(2), "{ended:?}");
    assert!(ended.stderr.contains("authority unqualified"), "{ended:?}");
    let mut config: Value = serde_json::from_slice(&fs::read(&fixture.config).unwrap()).unwrap();
    config["pipeline_uid"] = config["owner_uid"].clone();
    fs::write(&fixture.config, serde_json::to_vec(&config).unwrap()).unwrap();
    let ended = fixture
        .home
        .run(&["authority", "qualify", "--config", &fixture.config]);
    assert_eq!(ended.code, Some(2), "{ended:?}");
    assert!(ended.stderr.contains("authority unqualified"), "{ended:?}");
}

#[test]
#[ignore = "needs sudo: actual qualified host changes invalidate serving"]
fn n05_needs_sudo_qualification_binds_host_changes_and_private_store_files() {
    let fixture = Fixture::new();
    fixture.qualify();
    let config: Value = serde_json::from_slice(&fs::read(&fixture.config).unwrap()).unwrap();
    let launcher = Path::new(config["launcher"].as_str().unwrap());
    let receipt = Path::new(&fixture.store).join("qualification");
    let database = Path::new(&fixture.store).join("authority.sqlite3");
    // Even a qualified host cannot start on an unsafe database or receipt.
    fs::write(&database, "").unwrap();
    fs::set_permissions(&database, fs::Permissions::from_mode(0o644)).unwrap();
    let refused = fixture
        .home
        .run(&["authority", "serve", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    fs::remove_file(&database).unwrap();
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o644)).unwrap();
    let refused = fixture
        .home
        .run(&["authority", "serve", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o600)).unwrap();
    let original = fs::read(launcher).unwrap();
    fs::write(launcher, "#!/bin/sh\nexit 1\n").unwrap();
    let refused = fixture
        .home
        .run(&["authority", "serve", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    let refused = fixture
        .home
        .run(&["authority", "qualify", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(!receipt.exists(), "failed probes preserved qualification");
    // A launcher that damages the protected witness cannot attest successful separation.
    let damaging = str::from_utf8(&original).unwrap().replacen(
        "uid=$1",
        "printf changed > \"$6/probe-witness\"\nuid=$1",
        1,
    );
    fs::write(launcher, damaging).unwrap();
    let refused = fixture
        .home
        .run(&["authority", "qualify", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(!receipt.exists());
    fs::write(
        launcher,
        "#!/bin/sh\nprintf 'authority probe denied 65531\\n'\n",
    )
    .unwrap();
    let refused = fixture
        .home
        .run(&["authority", "qualify", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(!receipt.exists());
    fs::write(launcher, original).unwrap();
    // A substituted executable cannot impersonate the independently pinned probe.
    let binary = fixture.home.tools().join("authority/maestro");
    fs::write(
        &binary,
        "#!/bin/sh\nprintf 'authority probe denied %s\\n' \"$(id -u)\"\n",
    )
    .unwrap();
    let refused = fixture
        .home
        .run(&["authority", "qualify", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(!receipt.exists());
}

/// Independently authored hostile IPC client, launched only under the qualified fixture boundary.
#[test]
#[ignore = "fixture helper: needs the parent needs-sudo test's scratch socket and request bindings"]
fn n05_unprivileged_peer_uses_independent_socket_requests() {
    let endpoint = env::var("MAESTRO_N05_SOCKET").unwrap();
    let request = fs::read(env::var("MAESTRO_N05_REQUEST").unwrap()).unwrap();
    let mut socket = UnixStream::connect(endpoint).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    socket.write_all(&request).unwrap();
    socket.write_all(b"\n").unwrap();
    let mut reply = String::new();
    BufReader::new(socket).read_line(&mut reply).unwrap();
    println!("authority peer reply: {}", reply.trim());
}

#[test]
#[ignore = "needs sudo: read-only wire contract uses genuinely distinct peer identities"]
fn n05_needs_sudo_read_only_wire_replies_refuse_unknown_approval_fields() {
    use std::io::{BufRead as _, BufReader};
    use std::os::unix::net::UnixListener;
    // Explicit hostile wire-contract fixture, not a grant or qualification claim.
    for (reply, expected) in [
        (
            json!({"status":"permit","grant_id":"wire-fixture","model_approval":true}),
            Some(2),
        ),
        (
            json!({"status":"permit","grant_id":"wire-fixture"}),
            Some(0),
        ),
        (
            json!({"status":"expired","grant_id":"wire-fixture"}),
            Some(2),
        ),
    ] {
        let fixture = Fixture::new();
        let listener = UnixListener::bind(&fixture.socket).unwrap();
        fs::set_permissions(&fixture.socket, fs::Permissions::from_mode(0o666)).unwrap();
        let server = thread::spawn(move || {
            let (mut peer, _) = listener.accept().unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut request = String::new();
            BufReader::new(&peer).read_line(&mut request).unwrap();
            writeln!(peer, "{reply}").unwrap();
        });
        let grant = grant();
        let result = fixture.as_uid(
            "65534",
            &json!({
            "action":"decide",
            "principal":"65534",
            "operation":"fetch",
            "target":grant["target"]}),
        );
        assert_eq!(result.status.code(), expected, "{result:?}");
        server.join().unwrap();
    }
}
