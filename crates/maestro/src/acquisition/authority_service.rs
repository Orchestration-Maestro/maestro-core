//! Bounded Linux Unix-socket IPC authenticated with kernel peer credentials.
use super::{
    authority_host::{self, Host, unqualified},
    authority_store::Store,
};
use crate::failure::Failure;
use maestro_acquisition::policy::{
    authority::{Authority as _, AuthorityRefusal, Grant, Operation, Target},
    authority_socket::{AuthoritySocket, read_frame},
};
use rustix::net::sockopt::socket_peercred;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write as _,
    os::unix::{
        fs::PermissionsExt as _,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

/// Wire requests do not carry authenticated owner/model/manifest claims.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    /// Create or edit one exact grant, confirmed in full.
    Grant {
        /// Exact effect to grant or edit.
        grant: Grant,
        /// Owner-confirmed complete scope, target, identity and expiry.
        confirmation: Grant,
    },
    /// Revoke one exact existing grant, confirmed in full.
    Revoke {
        /// Exact stored grant to revoke.
        grant: Grant,
        /// Complete exact confirmation, not a generic yes.
        confirmation: Grant,
    },
    /// Read-only current decision; an unprivileged peer may ask only for itself.
    Decide {
        /// Platform-assigned principal, rechecked against the peer.
        principal: String,
        /// Separately authorized effect.
        operation: Operation,
        /// Exact conjunctive target.
        target: Target,
    },
}
/// A mutation reply contains no source content, secrets or arbitrary extra fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MutationResponse {
    /// Only an explicit `ok` acknowledges an audited mutation.
    status: String,
}
/// Unlink only the socket this serving invocation successfully bound.
struct SocketPath(PathBuf);
impl Drop for SocketPath {
    fn drop(&mut self) {
        drop(fs::remove_file(&self.0));
    }
}
/// Serve only a matching qualified host. Startup never launches or elevates processes.
pub(super) fn serve(
    path: &Path,
    ready: impl FnOnce() -> Result<(), Failure>,
) -> Result<Value, Failure> {
    let host = authority_host::checked(path)?;
    authority_host::qualified(&host)?;
    let mut store = Store::open(&host)?;
    let listener = UnixListener::bind(&host.socket).map_err(|_| unqualified())?;
    let _socket_path = SocketPath(host.socket.clone());
    fs::set_permissions(&host.socket, fs::Permissions::from_mode(0o666))
        .map_err(|_| unqualified())?;
    ready()?;
    for incoming in listener.incoming() {
        let mut stream = incoming.map_err(|_| unqualified())?;
        // A slow/hostile client owns neither an unbounded read nor the writer lock.
        let result =
            handle(&mut stream, &host, &mut store).unwrap_or_else(|_| json!({"status":"refused"}));
        drop(writeln!(stream, "{result}"));
    }
    Err(unqualified())
}
/// Authenticate before parsing any caller-supplied scope or approval.
fn handle(stream: &mut UnixStream, host: &Host, store: &mut Store) -> Result<Value, Failure> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| unqualified())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| unqualified())?;
    let peer = socket_peercred(&*stream)
        .map_err(|_| unqualified())?
        .uid
        .as_raw();
    let bytes = read_frame(stream).map_err(|_| Failure::refused("authority refused"))?;
    let request: Request =
        serde_json::from_slice(&bytes).map_err(|_| Failure::refused("authority refused"))?;
    match request {
        Request::Grant {
            grant,
            confirmation,
        } => change(host, store, peer, (&grant, &confirmation), false),
        Request::Revoke {
            grant,
            confirmation,
        } => change(host, store, peer, (&grant, &confirmation), true),
        Request::Decide {
            principal,
            operation,
            target,
        } => {
            if peer != host.owner_uid && principal != peer.to_string() {
                return Err(Failure::refused("authority refused"));
            }
            match store.decide(&principal, operation, &target, SystemTime::now()) {
                Ok(permit) => Ok(json!({"status":"permit","grant_id":permit.grant_id})),
                Err(AuthorityRefusal::Expired { grant_id }) => {
                    Ok(json!({"status":"expired","grant_id":grant_id}))
                }
                Err(AuthorityRefusal::Refused(_)) => Err(Failure::refused("authority refused")),
            }
        }
    }
}
/// The sole mutation route: exact confirmation AND actual owner peer identity.
fn change(
    host: &Host,
    store: &mut Store,
    peer: u32,
    confirmed: (&Grant, &Grant),
    revoke: bool,
) -> Result<Value, Failure> {
    let (grant, confirmation) = confirmed;
    let grant = grant
        .canonical()
        .map_err(|_| Failure::refused("authority refused"))?;
    let confirmation = confirmation
        .canonical()
        .map_err(|_| Failure::refused("authority refused"))?;
    if peer != host.owner_uid
        || grant != confirmation
        || (grant.principal != host.pipeline_uid.to_string()
            && grant.principal != host.connector_uid.to_string())
    {
        return Err(Failure::refused("authority refused"));
    }
    store
        .change(&grant, revoke, peer)
        .map_err(|_| Failure::refused("authority refused"))?;
    Ok(json!({"status":"ok"}))
}
/// Owner CLI and unprivileged clients authenticate the authority end as well.
pub(super) fn request(socket: &Path, authority_uid: u32, file: &Path) -> Result<Value, Failure> {
    let value: Request = authority_host::read(file)?;
    if let Request::Decide {
        principal,
        operation,
        target,
    } = &value
    {
        let authority = AuthoritySocket {
            socket: socket.to_owned(),
            authority_uid,
        };
        let permit = authority
            .decide(principal, *operation, target, SystemTime::now())
            .map_err(|_| Failure::refused("authority refused"))?;
        return Ok(json!({"status":"permit","grant_id":permit.grant_id}));
    }
    let bytes = serde_json::to_vec(&value).map_err(|_| Failure::refused("authority refused"))?;
    let mut stream = UnixStream::connect(socket).map_err(|_| unqualified())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| unqualified())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| unqualified())?;
    let peer = socket_peercred(&stream)
        .map_err(|_| unqualified())?
        .uid
        .as_raw();
    if peer != authority_uid {
        return Err(unqualified());
    }
    stream
        .write_all(&bytes)
        .and_then(|()| stream.write_all(b"\n"))
        .map_err(|_| unqualified())?;
    let response: MutationResponse =
        serde_json::from_slice(&read_frame(&mut stream).map_err(|_| unqualified())?)
            .map_err(|_| unqualified())?;
    if response.status != "ok" {
        return Err(Failure::refused("authority refused"));
    }
    Ok(json!({"status":"ok"}))
}
