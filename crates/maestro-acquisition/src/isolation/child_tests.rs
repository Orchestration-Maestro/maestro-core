//! Ordinary barrier-handshaken child processes exercise poll/read/wait and cleanup.
use super::port::Refusal;
use super::supervision::{Timing, cleanup, run};
use super::test_support::FixedClock;
use crate::{
    extraction::decode::{DecodeStage, ParserDecode},
    isolation::{
        cgroup::Delegation,
        test_support::{Groups, accounting},
    },
};
use maestro_kernel::retrieval::Clock;
use rustix::process::{Pid, Signal, kill_process};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, atomic::AtomicBool},
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use std::{
    sync::{Mutex, mpsc},
    thread,
};

fn child(script: &str) -> Child {
    Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}
#[test]
fn n17_child_eof_crash_done_truncated_and_missing_pipes() {
    for (script, expected) in [
        (
            "printf '%s\\n' '{\"kind\":\"ready\"}' '{\"kind\":\"done\"}'",
            Ok(vec![]),
        ),
        ("exit 0", Err(Refusal::Unsupported)),
        (
            "printf '%s\\n' '{\"kind\":\"unknown\"}'",
            Err(Refusal::Output),
        ),
        (
            "printf '%s\\n' '{\"kind\":\"ready\"}'; exit 7",
            Err(Refusal::Crash),
        ),
        (
            "printf '%s\\n' '{\"kind\":\"ready\"}'",
            Err(Refusal::Output),
        ),
        (
            "printf '%s\\n' '{\"kind\":\"ready\"}'; printf '{'",
            Err(Refusal::Output),
        ),
    ] {
        let mut ledger = accounting();
        let (clock, deadline) = ledger.read_timing().unwrap();
        let timing = Timing {
            clock,
            deadline,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let mut child = child(script);
        assert_eq!(
            run(
                &mut child,
                &mut ParserDecode::new(&mut ledger),
                &timing,
                4096
            ),
            expected,
            "{script}"
        );
        child.wait().unwrap();
    }
    let mut ledger = accounting();
    let (clock, deadline) = ledger.read_timing().unwrap();
    let timing = Timing {
        clock,
        deadline,
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let mut child = child("exit 0");
    drop(child.stdout.take());
    assert_eq!(
        run(
            &mut child,
            &mut ParserDecode::new(&mut ledger),
            &timing,
            4096
        ),
        Err(Refusal::Output)
    );
    child.wait().unwrap();
}
#[test]
fn n17_child_ack_handshake_and_finish_rechecks_prior_hold() {
    let script = concat!(
        "printf '%s\\n' '{\"kind\":\"ready\"}' '{\"kind\":\"decode\",\"value\":{",
        "\"stage\":\"office\",\"input_bytes\":10,\"expanded_bytes\":3,",
        "\"levels\":0,\"members\":0,\"entities\":0,\"pixels\":0,\"memory_bytes\":0}}'; ",
        "read -r ack; test \"$ack\" = OK || exit 9; ",
        "printf '%s\\n' '{\"kind\":\"data\",\"value\":\"abc\"}' '{\"kind\":\"done\"}'",
    );
    let mut ledger = accounting();
    let (clock, deadline) = ledger.read_timing().unwrap();
    let timing = Timing {
        clock,
        deadline,
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let mut child = child(script);
    assert_eq!(
        run(
            &mut child,
            &mut ParserDecode::new(&mut ledger),
            &timing,
            4096
        ),
        Ok(b"abc".to_vec())
    );
    child.wait().unwrap();
    let mut decode = ParserDecode::new(&mut ledger);
    decode.crashed(DecodeStage::Attachment);
    let mut child = self::child("printf '%s\\n' '{\"kind\":\"ready\"}' '{\"kind\":\"done\"}'");
    assert!(matches!(
        run(&mut child, &mut decode, &timing, 4096),
        Err(Refusal::Decode(_))
    ));
    child.wait().unwrap();
}
#[test]
fn n17_cleanup_reaps_leader_then_waits_for_group_and_propagates_effect_failure() {
    for fail in [None, Some(1), Some(2)] {
        let io = Groups::new();
        let delegation =
            Delegation::with_io(PathBuf::from("/sys/fs/cgroup/n17.service"), io.clone()).unwrap();
        let worker = delegation
            .worker("worker", &accounting().limits().clone(), 2)
            .unwrap();
        io.calls.lock().unwrap().clear();
        *io.fail.lock().unwrap() = fail;
        io.events.lock().unwrap().extend([
            "populated 1".into(),
            "populated 1".into(),
            "populated 0".into(),
        ]);
        // Blocks until EOF/kill, never a sleep-based success race.
        let mut child = child("read -r barrier");
        assert_eq!(
            cleanup(&worker, &mut child),
            if fail.is_some() {
                Err(Refusal::Cleanup)
            } else {
                Ok(())
            }
        );
        assert!(
            child.try_wait().unwrap().is_some(),
            "leader must be reaped even on group-kill failure"
        );
        assert_eq!(
            io.calls.lock().unwrap().len(),
            match fail {
                None => 4,
                Some(1) => 1,
                _ => 2,
            }
        );
    }
}

/// First quantum is live; the next reaches the same anchored cutoff exactly.
#[derive(Debug)]
struct CutoffClock {
    now: Instant,
    calls: AtomicUsize,
}
impl Clock for CutoffClock {
    fn now(&self) -> Instant {
        if self.calls.fetch_add(1, Ordering::AcqRel) == 0 {
            self.now
        } else {
            self.now + Duration::from_secs(1)
        }
    }
}
#[test]
fn n17_child_pending_pipe_returns_to_anchored_deadline_without_output() {
    bounded_pending_child("read -r barrier", false);
}

#[test]
fn n17_r1_guard_child_reaping_and_live_eof() {
    let io = Groups::new();
    let delegation =
        Delegation::with_io(PathBuf::from("/sys/fs/cgroup/n17.service"), io.clone()).unwrap();
    let worker = delegation
        .worker("worker", accounting().limits(), 2)
        .unwrap();
    let mut leader = child("read -r barrier");
    let pid = leader.id();
    *io.reaping_pid.lock().unwrap() = Some(pid);
    cleanup(&worker, &mut leader).unwrap();
    assert!(
        !PathBuf::from(format!("/proc/{pid}")).exists(),
        "leader must already be reaped"
    );
    bounded_pending_child("exec 1>&-; read -r barrier", true);
}

#[test]
fn n17_child_pending_poll_and_deadline_equality_fail_fast() {
    bounded_pending_child("read -r barrier", false);
    let now = Instant::now();
    let timing = Timing {
        deadline: now,
        clock: Arc::new(FixedClock(Mutex::new(now))),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let mut ledger = accounting();
    let mut leader = child("read -r barrier");
    assert_eq!(
        run(
            &mut leader,
            &mut ParserDecode::new(&mut ledger),
            &timing,
            4096
        ),
        Err(Refusal::Timeout)
    );
    leader.kill().unwrap();
    leader.wait().unwrap();
}

/// For live EOF, let the driver observe HUP and one live EOF wait before cutoff.
#[derive(Debug)]
struct EofClock {
    pid: u32,
    now: Instant,
    closed: AtomicUsize,
}
impl Clock for EofClock {
    fn now(&self) -> Instant {
        if !PathBuf::from(format!("/proc/{}/fd/1", self.pid)).exists()
            && self.closed.fetch_add(1, Ordering::AcqRel) >= 3
        {
            self.now + Duration::from_secs(1)
        } else {
            self.now
        }
    }
}

/// A lost poll wakeup must fail an assertion; kill releases the blocked read.
fn bounded_pending_child(script: &str, live_eof: bool) {
    let now = Instant::now();
    let mut leader = child(script);
    let pid = leader.id();
    let clock: Arc<dyn Clock> = if live_eof {
        Arc::new(EofClock {
            pid,
            now,
            closed: AtomicUsize::new(0),
        })
    } else {
        Arc::new(CutoffClock {
            now,
            calls: AtomicUsize::new(0),
        })
    };
    let (send, receive) = mpsc::channel();
    let task = thread::spawn(move || {
        let mut ledger = accounting();
        let timing = Timing {
            deadline: now + Duration::from_secs(1),
            clock,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let result = run(
            &mut leader,
            &mut ParserDecode::new(&mut ledger),
            &timing,
            4096,
        );
        send.send(result).unwrap();
        leader.kill().unwrap();
        leader.wait().unwrap();
    });
    let observed = receive.recv_timeout(Duration::from_secs(5));
    if observed.is_err() {
        kill_process(
            Pid::from_raw(i32::try_from(pid).unwrap()).unwrap(),
            Signal::KILL,
        )
        .unwrap();
    }
    task.join().unwrap();
    assert_eq!(
        observed,
        Ok(Err(Refusal::Timeout)),
        "lost wakeup must return to the cutoff"
    );
}
