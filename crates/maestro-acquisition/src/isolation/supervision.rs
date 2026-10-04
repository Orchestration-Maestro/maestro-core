//! Bounded parent-side parser IPC and owned whole-tree teardown.
use super::{cgroup::Worker, port::Refusal};
use crate::extraction::decode::{DecodeRequest, ParserDecode};
use maestro_kernel::retrieval::{Clock, SystemClock};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use serde::Deserialize;
use std::{
    io::{Read as _, Write},
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
    fn frame(
        &mut self,
        decode: &mut ParserDecode<'_>,
        ack: &mut Option<&mut dyn Write>,
    ) -> Result<(), Refusal> {
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
                ack.as_mut()
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
        mut ack: Option<&mut dyn Write>,
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
                self.frame(decode, &mut ack)?;
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
        Ok(remaining.min(Duration::from_millis(10)))
    }
}
/// Only requested readable/hangup readiness may enter a blocking read.
const READABLE: PollFlags = PollFlags::IN.union(PollFlags::HUP);
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
        if duration.is_zero() {
            return Err(Refusal::Timeout);
        }
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
        let mut pending = [PollFd::new(stdout, READABLE)];
        poll(&mut pending, Some(&quantum)).map_err(|_| Refusal::Containment)?;
        if pending[0].revents().intersects(READABLE) {
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
                child.stdin.as_mut().map(|stdin| stdin as &mut dyn Write),
            )?;
        }
    }
}
/// Whole-tree kill, kernel emptiness and reaping precede cgroup/scratch deletion.
/// Collection reuses the N17 §4 120-second ceiling; Pi has no kernel-handshake equivalent.
pub(super) fn cleanup(worker: &Worker, child: &mut Child) -> Result<(), Refusal> {
    let killed = worker.kill();
    // Attach may have failed while the bootstrap was still outside the leaf.
    // A failed leader kill is harmless only when wait proves it already exited.
    let _leader = child.kill();
    child.wait().map_err(|_| Refusal::Cleanup)?;
    killed?;
    // SIGKILL tree teardown is asynchronous. Never substitute leader exit for emptiness.
    wait_empty(
        || worker.empty(),
        Instant::now() + Duration::from_mins(2),
        &SystemClock,
    )
}
/// Private deadline/clock seam for asynchronous kernel emptiness observations.
pub(super) fn wait_empty(
    mut empty: impl FnMut() -> Result<bool, Refusal>,
    deadline: Instant,
    clock: &dyn Clock,
) -> Result<(), Refusal> {
    while !empty()? {
        let remaining = deadline
            .checked_duration_since(clock.now())
            .unwrap_or_default();
        if remaining.is_zero() {
            return Err(Refusal::Cleanup);
        }
        thread::sleep(remaining.min(Duration::from_millis(10)));
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

#[cfg(test)]
mod protocol_tests {
    use super::{Protocol, Refusal, Timing};
    use crate::{
        extraction::decode::{DecodeRequest, DecodeStage, ParserDecode},
        isolation::test_support::{FixedClock, accounting},
    };
    use std::sync::{Mutex, atomic::Ordering};
    use std::{
        io,
        sync::{Arc, atomic::AtomicBool},
        time::{Duration, Instant},
    };

    fn protocol() -> Protocol {
        Protocol {
            frame: vec![],
            output: vec![],
            reserved: 0,
            ready: false,
            done: false,
            wire: 0,
        }
    }
    fn proposal() -> Vec<u8> {
        let request = DecodeRequest {
            stage: DecodeStage::Office,
            input_bytes: 10,
            expanded_bytes: 10,
            levels: 0,
            members: 0,
            entities: 0,
            pixels: 0,
            memory_bytes: 0,
        };
        let mut wire =
            serde_json::to_vec(&serde_json::json!({"kind": "decode", "value": request})).unwrap();
        wire.push(b'\n');
        wire
    }
    #[test]
    fn n17_r1_guard_missing_ack() {
        let mut ledger = accounting();
        let mut decode = ParserDecode::new(&mut ledger);
        let mut state = protocol();
        state
            .bytes(b"{\"kind\":\"ready\"}\n", 4096, &mut decode, None)
            .unwrap();
        assert_eq!(
            state.bytes(&proposal(), 4096, &mut decode, None),
            Err(Refusal::Output)
        );
        assert!(state.output.is_empty());
        assert_eq!(decode.receipts().len(), 1);
    }

    #[test]
    fn n17_protocol_fragments_charge_ack_then_data_done() {
        let mut ledger = accounting();
        let mut decode = ParserDecode::new(&mut ledger);
        let mut state = protocol();
        let mut ack = vec![];
        let mut wire = b"{\"kind\":\"ready\"}\n".to_vec();
        wire.extend(proposal());
        wire.extend(b"{\"kind\":\"data\",\"value\":\"1234567890\"}\n{\"kind\":\"done\"}\n");
        for fragment in wire.chunks(3) {
            state
                .bytes(fragment, wire.len() as u64, &mut decode, Some(&mut ack))
                .unwrap();
        }
        assert_eq!(ack, b"OK\n");
        assert_eq!(state.output, b"1234567890");
        assert!(state.ready && state.done && state.frame.is_empty());
        assert_eq!(state.reserved, 0);
        assert_eq!(state.wire, wire.len() as u64);
        assert_eq!(decode.finish().unwrap().len(), 1);
        assert_eq!(ledger.expanded_bytes(), 10);
    }
    #[test]
    fn n17_protocol_refusals_keep_output_held_and_caps_exact() {
        for (wire, expected) in [
            (
                "{\"kind\":\"data\",\"value\":\"x\"}\n",
                Refusal::Unsupported,
            ),
            ("{\"kind\":\"done\"}\n", Refusal::Unsupported),
            ("{\"kind\":\"decode\",\"value\":{}}\n", Refusal::Output),
            (
                "{\"kind\":\"ready\"}\n{\"kind\":\"ready\"}\n",
                Refusal::Output,
            ),
            (
                "{\"kind\":\"ready\"}\n{\"kind\":\"data\",\"value\":\"x\"}\n",
                Refusal::Output,
            ),
            (
                "{\"kind\":\"ready\"}\n{\"kind\":\"done\"}\nx",
                Refusal::Output,
            ),
            ("{\"kind\":\"unknown\"}\n", Refusal::Output),
            ("{\"kind\":\"ready\",\"extra\":1}\n", Refusal::Output),
            ("garbage\n", Refusal::Output),
        ] {
            let mut ledger = accounting();
            assert_eq!(
                protocol().bytes(
                    wire.as_bytes(),
                    4096,
                    &mut ParserDecode::new(&mut ledger),
                    Some(&mut vec![])
                ),
                Err(expected),
                "{wire}"
            );
        }
        let mut ledger = accounting();
        let mut decode = ParserDecode::new(&mut ledger);
        let wire = b"{\"kind\":\"ready\"}\n";
        assert!(
            protocol()
                .bytes(wire, wire.len() as u64, &mut decode, Some(&mut vec![]))
                .is_ok()
        );
        assert_eq!(
            protocol().bytes(wire, wire.len() as u64 - 1, &mut decode, Some(&mut vec![])),
            Err(Refusal::Output)
        );
        let mut state = protocol();
        state.wire = u64::MAX;
        assert_eq!(
            state.bytes(b"x", u64::MAX, &mut decode, Some(&mut vec![])),
            Err(Refusal::Output)
        );
        let mut state = protocol();
        state.frame = vec![b'x'; 4 * 1024 * 1024];
        assert_eq!(
            state.bytes(b"x", u64::MAX, &mut decode, Some(&mut vec![])),
            Err(Refusal::Output)
        );
        let mut state = protocol();
        state
            .bytes(b"{", 1, &mut decode, Some(&mut vec![]))
            .unwrap();
        assert_eq!(state.frame, b"{");
    }
    #[test]
    fn n17_protocol_frame_ceiling_accepts_its_last_byte() {
        let mut ledger = accounting();
        let mut decode = ParserDecode::new(&mut ledger);
        let mut state = protocol();
        state.frame = vec![b'x'; 4 * 1024 * 1024 - 1];
        assert_eq!(
            state.bytes(b"x", u64::MAX, &mut decode, Some(&mut vec![])),
            Ok(())
        );
        assert_eq!(state.frame.len(), 4 * 1024 * 1024);
        assert_eq!(
            state.bytes(b"x", u64::MAX, &mut decode, Some(&mut vec![])),
            Err(Refusal::Output)
        );
    }
    /// Failed barrier acknowledgement must never admit data to the caller.
    struct BrokenAck;
    impl io::Write for BrokenAck {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn n17_protocol_ack_overflow_pre_ready_and_cumulative_n16_refusals() {
        let mut ledger = accounting();
        let mut decode = ParserDecode::new(&mut ledger);
        assert_eq!(
            protocol().bytes(&proposal(), 4096, &mut decode, Some(&mut vec![])),
            Err(Refusal::Unsupported)
        );
        let mut state = protocol();
        state.ready = true;
        assert_eq!(
            state.bytes(&proposal(), 4096, &mut decode, Some(&mut BrokenAck)),
            Err(Refusal::Output)
        );
        assert_eq!(decode.receipts().len(), 1);
        let mut state = protocol();
        state.ready = true;
        state.reserved = u64::MAX;
        assert_eq!(
            state.bytes(&proposal(), 4096, &mut decode, Some(&mut vec![])),
            Err(Refusal::Output)
        );
        let mut state = protocol();
        state.ready = true;
        let mut ack = vec![];
        for _ in 0..8 {
            state
                .bytes(&proposal(), 4096, &mut decode, Some(&mut ack))
                .unwrap();
        }
        assert_eq!(decode.receipts().len(), 10);
        let prior = ack.clone();
        assert!(matches!(
            state.bytes(&proposal(), 4096, &mut decode, Some(&mut ack)),
            Err(Refusal::Decode(_))
        ));
        assert_eq!(ack, prior);
        assert!(decode.finish().is_err());
    }
    #[test]
    fn n17_quantum_deadline_equality_and_cancellation_precedence() {
        let now = Instant::now();
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut timing = Timing {
            deadline: now + Duration::from_millis(11),
            clock: Arc::new(FixedClock(Mutex::new(now))),
            cancelled: cancelled.clone(),
        };
        assert_eq!(timing.quantum(), Ok(Duration::from_millis(10)));
        timing.deadline = now + Duration::from_millis(1);
        assert_eq!(timing.quantum(), Ok(Duration::from_millis(1)));
        timing.deadline = now;
        assert_eq!(timing.quantum(), Ok(Duration::ZERO));
        timing.deadline = now.checked_sub(Duration::from_millis(1)).unwrap();
        assert_eq!(timing.quantum(), Err(Refusal::Timeout));
        cancelled.store(true, Ordering::Release);
        assert_eq!(timing.quantum(), Err(Refusal::Cancelled));
    }
}
