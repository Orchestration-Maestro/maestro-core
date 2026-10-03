//! Bounded parent-side parser IPC and owned whole-tree teardown.
use super::{cgroup::Worker, port::Refusal};
use crate::extraction::decode::{DecodeRequest, ParserDecode};
use maestro_kernel::retrieval::Clock;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use serde::Deserialize;
use std::{
    io::{Read as _, Write as _},
    process::Child,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// Strict JSON-lines protocol: acknowledge preflight before emitting admitted data.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Frame {
    /// Trusted bootstrap completed every kernel control before native exec.
    Ready,
    /// Charge the shared ledger before any parser expansion.
    Decode(DecodeRequest),
    /// Output cannot exceed this process's acknowledged expansion reservations.
    Data(String),
    /// Exactly one terminal frame; success also requires a successful process exit.
    Done,
}
/// One run's framing/reservation state; no output escapes on a held result.
struct Protocol {
    /// Incremental untrusted frame, bounded before every append.
    frame: Vec<u8>,
    /// Admitted output, held until exit, finish and cleanup all pass.
    output: Vec<u8>,
    /// Output reservation acknowledged in this IPC session, never prior HTTP costs.
    reserved: u64,
    /// Trusted bootstrap completed all required mechanisms.
    ready: bool,
    /// Terminal frame seen; subsequent bytes refuse.
    done: bool,
    /// Total wire bytes, including hostile preflight/JSON overhead.
    wire: u64,
}
impl Protocol {
    /// No parser output can count as a successful bootstrap/capability probe.
    fn prepared(&self) -> Result<(), Refusal> {
        if !self.ready {
            return Err(Refusal::Unsupported);
        }
        Ok(())
    }
    /// Consume a strict frame only after it fits the defensive format ceiling.
    fn frame(&mut self, decode: &mut ParserDecode<'_>, child: &mut Child) -> Result<(), Refusal> {
        let frame: Frame = serde_json::from_slice(&self.frame).map_err(|_| Refusal::Output)?;
        self.frame.clear();
        match frame {
            Frame::Ready => {
                if self.ready {
                    return Err(Refusal::Output);
                }
                self.ready = true;
            }
            Frame::Decode(request) => {
                self.prepared()?;
                let bytes = request.expanded_bytes;
                decode.admit(request).map_err(Refusal::Decode)?;
                self.reserved = self.reserved.checked_add(bytes).ok_or(Refusal::Output)?;
                child
                    .stdin
                    .as_mut()
                    .ok_or(Refusal::Output)?
                    .write_all(b"OK\n")
                    .map_err(|_| Refusal::Output)?;
            }
            Frame::Data(data) => {
                self.prepared()?;
                self.data(&data)?;
            }
            Frame::Done => {
                self.prepared()?;
                self.done = true;
            }
        }
        Ok(())
    }
    /// Data cannot borrow a prior stage's budget or exceed acknowledged reservations.
    fn data(&mut self, data: &str) -> Result<(), Refusal> {
        self.reserved = self
            .reserved
            .checked_sub(data.len() as u64)
            .ok_or(Refusal::Output)?;
        self.output
            .try_reserve_exact(data.len())
            .map_err(|_| Refusal::Output)?;
        self.output.extend_from_slice(data.as_bytes());
        Ok(())
    }
    /// No read, append or parse may grow beyond the core-owned ceilings.
    fn bytes(
        &mut self,
        bytes: &[u8],
        cap: u64,
        decode: &mut ParserDecode<'_>,
        child: &mut Child,
    ) -> Result<(), Refusal> {
        self.wire = self
            .wire
            .checked_add(bytes.len() as u64)
            .ok_or(Refusal::Output)?;
        if self.wire > cap {
            return Err(Refusal::Output);
        }
        for byte in bytes {
            if self.done {
                return Err(Refusal::Output);
            }
            if *byte == b'\n' {
                self.frame(decode, child)?;
                continue;
            }
            if self.frame.len() as u64 >= cap.min(4 * 1024 * 1024) {
                return Err(Refusal::Output);
            }
            self.frame.push(*byte);
        }
        Ok(())
    }
}
/// An explicit owned cutoff and caller cancellation, checked on each poll quantum.
pub(super) struct Timing {
    /// Original cumulative document cutoff; never restarted at parser handoff.
    pub(super) deadline: Instant,
    /// Same trusted clock as N09 accounting.
    pub(super) clock: Arc<dyn Clock>,
    /// Core-owned cancellation.
    pub(super) cancelled: Arc<AtomicBool>,
}
impl Timing {
    /// Internal 10-ms scheduling quantum is not a configurable/default run budget.
    fn quantum(&self) -> Result<Duration, Refusal> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(Refusal::Cancelled);
        }
        let remaining = self
            .deadline
            .checked_duration_since(self.clock.now())
            .ok_or(Refusal::Timeout)?;
        if remaining.is_zero() {
            return Err(Refusal::Timeout);
        }
        Ok(remaining.min(Duration::from_millis(10)))
    }
}
/// Read bounded protocol frames and wait for successful parser/namespace exit.
pub(super) fn run(
    child: &mut Child,
    decode: &mut ParserDecode<'_>,
    timing: &Timing,
    cap: u64,
) -> Result<Vec<u8>, Refusal> {
    let mut protocol = Protocol {
        frame: Vec::new(),
        output: Vec::new(),
        reserved: 0,
        done: false,
        ready: false,
        wire: 0,
    };
    let mut eof = false;
    loop {
        let duration = timing.quantum()?;
        if eof {
            let Some(status) = child.try_wait().map_err(|_| Refusal::Crash)? else {
                thread::sleep(duration);
                continue;
            };
            if !protocol.ready {
                return Err(Refusal::Unsupported);
            }
            if !status.success() {
                return Err(Refusal::Crash);
            }
            if !protocol.done || !protocol.frame.is_empty() {
                return Err(Refusal::Output);
            }
            decode.finish().map_err(Refusal::Decode)?;
            return Ok(protocol.output);
        }
        let quantum: Timespec = duration.try_into().map_err(|_| Refusal::Configuration)?;
        let stdout = child.stdout.as_ref().ok_or(Refusal::Output)?;
        let mut pending = [PollFd::new(stdout, PollFlags::IN | PollFlags::HUP)];
        poll(&mut pending, Some(&quantum)).map_err(|_| Refusal::Containment)?;
        if !pending[0].revents().is_empty() {
            let mut buffer = [0_u8; 4096];
            let count = child
                .stdout
                .as_mut()
                .ok_or(Refusal::Output)?
                .read(&mut buffer)
                .map_err(|_| Refusal::Output)?;
            if count == 0 {
                eof = true;
                continue;
            }
            protocol.bytes(
                buffer.get(..count).ok_or(Refusal::Output)?,
                cap,
                decode,
                child,
            )?;
        }
    }
}
/// Whole-tree kill, kernel emptiness and reaping precede cgroup/scratch deletion.
pub(super) fn cleanup(worker: &Worker, child: &mut Child) -> Result<(), Refusal> {
    let killed = worker.kill();
    // Attach may have failed while the bootstrap was still outside the leaf.
    // A failed leader kill is harmless only when wait proves it already exited.
    let _leader = child.kill();
    child.wait().map_err(|_| Refusal::Cleanup)?;
    killed?;
    // SIGKILL tree teardown is asynchronous. Never substitute leader exit for emptiness.
    while !worker.empty()? {
        thread::yield_now();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Frame, Protocol, Refusal};
    #[test]
    fn n17_default_ipc_shapes_handshake_and_exact_output_reservations() {
        let mut protocol = Protocol {
            frame: Vec::new(),
            output: Vec::new(),
            reserved: 10,
            ready: false,
            done: false,
            wire: 0,
        };
        assert_eq!(protocol.prepared(), Err(Refusal::Unsupported));
        protocol.ready = true;
        assert_eq!(protocol.prepared(), Ok(()));
        protocol.data("12345678").unwrap();
        assert_eq!(protocol.reserved, 2);
        protocol.data("90").unwrap();
        assert_eq!(protocol.reserved, 0);
        assert_eq!(protocol.output, b"1234567890");
        assert_eq!(protocol.data("extra"), Err(Refusal::Output));
        assert_eq!(protocol.output, b"1234567890");
        for invalid in [
            r#"{"kind":"done","unexpected":1}"#,
            r#"{"kind":"data","value":1}"#,
            r#"{"kind":"unknown"}"#,
            r#"{"kind":"decode","value":{}}"#,
            r#"{"kind":"ready","value":"extra"}"#,
        ] {
            assert!(serde_json::from_str::<Frame>(invalid).is_err());
        }
        assert!(serde_json::from_str::<Frame>(r#"{"kind":"done"}"#).is_ok());
        assert!(serde_json::from_str::<Frame>(r#"{"kind":"ready"}"#).is_ok());
    }
}
