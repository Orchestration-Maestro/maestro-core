//! Read-only local IPC adapter: both peers are kernel-authenticated, never JSON identities.
use super::authority::{Authority, AuthorityRefusal, Operation, Permit, Target};
use crate::Refusal;
#[cfg(target_os = "linux")]
use maestro_kernel::retrieval::{Clock, SystemClock};
#[cfg(target_os = "linux")]
use std::{
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};
use std::{path::PathBuf, time::SystemTime};

/// Platform-local read-only decisions; unsupported hosts report unqualified.
#[derive(Debug)]
pub struct AuthoritySocket {
    /// Trusted local endpoint, not a source-manifest field.
    pub socket: PathBuf,
    /// Expected authority service identity, supplied by authorized host setup.
    pub authority_uid: u32,
}
impl Authority for AuthoritySocket {
    fn decide(
        &self,
        principal: &str,
        operation: Operation,
        target: &Target,
        _now: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        #[cfg(target_os = "linux")]
        {
            decision(self, principal, operation, target)
        }
        #[cfg(not(target_os = "linux"))]
        {
            // No qualified IPC/authentication adapter exists for these hosts yet.
            let _ = (principal, operation, target);
            Err(Refusal::Unqualified.into())
        }
    }
}
/// The server rechecks its current clock and live store for every dispatch.
#[cfg(target_os = "linux")]
fn decision(
    authority: &AuthoritySocket,
    principal: &str,
    operation: Operation,
    target: &Target,
) -> Result<Permit, AuthorityRefusal> {
    use rustix::{net::sockopt::socket_peercred, process::geteuid};
    use serde::Deserialize;
    use serde_json::json;
    use std::{io::Write as _, time::Duration};
    /// Bounded response; a refusal carries no grant/source data.
    #[derive(Deserialize)]
    #[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
    enum Response {
        /// Exact currently matched grant.
        Permit {
            /// Opaque immutable audit identity.
            grant_id: String,
        },
        /// Exact matched authority expired; the port itself writes no receipt.
        Expired {
            /// Exact expired grant identity.
            grant_id: String,
        },
        /// Current authority does not permit this request.
        Refused,
    }
    target.validate()?;
    let uid = geteuid().as_raw();
    if uid == authority.authority_uid || principal != uid.to_string() {
        return Err(Refusal::Access.into());
    }
    let mut socket = UnixStream::connect(&authority.socket).map_err(|_| Refusal::Unqualified)?;
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| Refusal::Unqualified)?;
    socket
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| Refusal::Unqualified)?;
    if socket_peercred(&socket)
        .map_err(|_| Refusal::Unqualified)?
        .uid
        .as_raw()
        != authority.authority_uid
    {
        return Err(Refusal::Unqualified.into());
    }
    let request =
        json!({"action":"decide","principal":principal,"operation":operation,"target":target});
    writeln!(socket, "{request}").map_err(|_| Refusal::Unqualified)?;
    let bytes = read_frame(&mut socket)?;
    match serde_json::from_slice(&bytes).map_err(|_| Refusal::Unqualified)? {
        Response::Permit { grant_id } => Ok(Permit { grant_id }),
        Response::Expired { grant_id } => Err(AuthorityRefusal::Expired { grant_id }),
        Response::Refused => Err(Refusal::Access.into()),
    }
}

/// Read one complete 64 KiB local IPC frame within a cumulative two-second deadline.
///
/// # Errors
/// Oversized, unterminated, disconnected or trickle-fed frames refuse.
#[cfg(target_os = "linux")]
pub fn read_frame(socket: &mut UnixStream) -> Result<Vec<u8>, Refusal> {
    read_frame_with_clock(socket, &SystemClock)
}

/// Positive remaining IPC budget handed to the OS, never a reset or zero timeout.
///
/// # Errors
/// A reached or exhausted absolute deadline refuses as `Deadline`.
#[cfg(target_os = "linux")]
pub fn frame_timeout(deadline: Instant, now: Instant) -> Result<Duration, Refusal> {
    if now >= deadline {
        return Err(Refusal::Deadline);
    }
    Ok(deadline.duration_since(now))
}

/// Read the same bounded real socket using a replaceable monotonic clock.
///
/// # Errors
/// Frame violations refuse; cumulative budget exhaustion is a typed `Deadline`.
#[cfg(target_os = "linux")]
pub fn read_frame_with_clock(
    socket: &mut UnixStream,
    clock: &dyn Clock,
) -> Result<Vec<u8>, Refusal> {
    use std::io::Read as _;
    let deadline = clock.now() + Duration::from_secs(2);
    let mut bytes = Vec::with_capacity(65_536);
    let mut buffer = [0; 4096];
    loop {
        let left = frame_timeout(deadline, clock.now())?;
        socket
            .set_read_timeout(Some(left))
            .map_err(|_| Refusal::Unqualified)?;
        let count = socket.read(&mut buffer).map_err(|_| Refusal::Unqualified)?;
        if count == 0 {
            return Err(Refusal::Unqualified);
        }
        if bytes.len() + count > 65_536 {
            return Err(Refusal::Unqualified);
        }
        bytes.extend_from_slice(buffer.get(..count).ok_or(Refusal::Unqualified)?);
        if bytes.contains(&b'\n') {
            if bytes.last() != Some(&b'\n') {
                return Err(Refusal::Unqualified);
            }
            return Ok(bytes);
        }
    }
}
