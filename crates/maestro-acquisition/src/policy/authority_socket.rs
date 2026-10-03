//! Read-only local IPC adapter: both peers are kernel-authenticated, never JSON identities.
use super::authority::{Authority, AuthorityRefusal, Operation, Permit, Target};
use crate::Refusal;
#[cfg(target_os = "linux")]
use maestro_kernel::retrieval::{Clock, SystemClock};
#[cfg(target_os = "linux")]
use std::{
    os::unix::net::UnixStream,
    path::Path,
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
    let mut socket = connect_bounded(
        &authority.socket,
        SystemClock.now() + Duration::from_secs(3),
        &SystemClock,
    )?;
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

/// Nonblocking Linux IPC connect with a trusted absolute deadline.
/// No authority decision or credential bytes are sent by this helper.
///
/// # Errors
/// Saturated backlogs and in-progress connects are bounded; expiry is `Deadline`.
#[cfg(target_os = "linux")]
pub fn connect_bounded(
    path: &Path,
    deadline: Instant,
    clock: &dyn Clock,
) -> Result<UnixStream, Refusal> {
    use rustix::{
        event::{PollFd, PollFlags, Timespec, poll},
        fs::{OFlags, fcntl_getfl, fcntl_setfl},
        io::Errno,
        net::{
            AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with,
            sockopt::socket_error,
        },
    };
    use std::slice::from_mut;
    let address = SocketAddrUnix::new(path).map_err(|_| Refusal::Unqualified)?;
    let socket = socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
        None,
    )
    .map_err(|_| Refusal::Unqualified)?;
    loop {
        let left = frame_timeout(deadline, clock.now())?;
        let connected = connect(&socket, &address);
        if connected.is_ok() || connected == Err(Errno::ISCONN) {
            break;
        }
        if connected != Err(Errno::AGAIN) && connected != Err(Errno::INPROGRESS) {
            return Err(Refusal::Unqualified);
        }
        let term = Timespec::try_from(left).map_err(|_| Refusal::Unqualified)?;
        let mut fd = PollFd::new(&socket, PollFlags::OUT);
        let polled = poll(from_mut(&mut fd), Some(&term));
        if polled == Err(Errno::INTR) {
            continue;
        }
        let count = polled.map_err(|_| Refusal::Unqualified)?;
        frame_timeout(deadline, clock.now())?;
        if count == 0 {
            return Err(Refusal::Deadline);
        }
        socket_error(&socket)
            .map_err(|_| Refusal::Unqualified)?
            .map_err(|_| Refusal::Unqualified)?;
        if fd
            .revents()
            .intersects(PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL)
        {
            return Err(Refusal::Unqualified);
        }
        if connected == Err(Errno::INPROGRESS) {
            break;
        }
        // Unix EAGAIN did not start a connect: retry only after the bounded poll.
    }
    frame_timeout(deadline, clock.now())?;
    let flags = fcntl_getfl(&socket).map_err(|_| Refusal::Unqualified)?;
    fcntl_setfl(&socket, flags & !OFlags::NONBLOCK).map_err(|_| Refusal::Unqualified)?;
    Ok(UnixStream::from(socket))
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
        frame_timeout(deadline, clock.now())?;
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

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use super::{Clock, Duration, Instant, UnixStream, read_frame_with_clock};
    use crate::Refusal;
    use std::{
        io::Write as _,
        sync::atomic::{AtomicUsize, Ordering},
    };

    /// Deterministic observations immediately before and at the cumulative cutoff.
    #[derive(Debug)]
    struct ProbeClock {
        t0: Instant,
        calls: AtomicUsize,
    }

    impl Clock for ProbeClock {
        fn now(&self) -> Instant {
            let elapsed = match self.calls.fetch_add(1, Ordering::SeqCst) {
                0 => Duration::ZERO,
                1 => Duration::from_millis(1_999),
                _ => Duration::from_secs(2),
            };
            self.t0 + elapsed
        }
    }

    #[test]
    fn n05_probe_completed_frame_at_cumulative_deadline_refuses() {
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(b"{}\n").unwrap();
        let clock = ProbeClock {
            t0: Instant::now(),
            calls: AtomicUsize::new(0),
        };
        let result = read_frame_with_clock(&mut reader, &clock);
        assert_eq!(result, Err(Refusal::Deadline));
        assert_eq!(clock.calls.load(Ordering::SeqCst), 3);
    }
}
