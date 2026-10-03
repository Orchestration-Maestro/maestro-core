//! Owner-approved real Linux effects; absence of provisioning fails this suite.
use super::n17_support::{self as fixture, accounting, clean, required};
use maestro_acquisition::{
    isolation::port::{Isolation, Refusal},
    transport::stream::{Accounting, Failure},
};
use maestro_kernel::artifact::Digest;
use std::{
    env,
    fs::{self, File},
    path::Path,
    sync::{Arc, atomic::Ordering},
    thread,
    time::{Duration, Instant},
};

/// Observe a real kernel counter while the worker still owns its bounded leaf.
fn observe(group: &Path, file: &str, field: &str, deadline: Instant) -> bool {
    while Instant::now() < deadline {
        for entry in fs::read_dir(group).unwrap() {
            let entry = entry.unwrap();
            if !entry.file_name().to_string_lossy().starts_with("n17-") {
                continue;
            }
            let Ok(value) = fs::read_to_string(entry.path().join(file)) else {
                continue;
            };
            if value.lines().any(|line| {
                line.strip_prefix(field)
                    .is_some_and(|count| count.trim().parse::<u64>().unwrap() > 0)
            }) {
                return true;
            }
        }
        thread::park_timeout(Duration::from_millis(5));
    }
    false
}
/// One real unit, with a fresh owned worker leaf and scratch for every exit path.
#[test]
fn n17_real_kernel_controls_cleanup_and_supervisor_recovery() {
    assert_eq!(
        env::var("MAESTRO_N17_REQUIRED").as_deref(),
        Ok("1"),
        "real containment qualification requires MAESTRO_N17_REQUIRED=1"
    );
    let group = fixture::delegated();
    let scratch = required("MAESTRO_N17_SCRATCH");
    let adapter = fixture::host(&scratch, &group);
    let binary = required("MAESTRO_N17_STATIC");
    let dynamic = required("MAESTRO_N17_DYNAMIC");
    let executable_memory = required("MAESTRO_N17_WX");
    let canary = required("MAESTRO_N17_CANARY");
    assert!(canary.is_file());
    if env::var("MAESTRO_N17_DEATH_PHASE").as_deref() == Ok("preparing") {
        fixture::hold_preparing(&scratch);
    }
    // The external scoped provisioner SIGKILLs this deliberately blocked supervisor.
    if env::var("MAESTRO_N17_DEATH_PHASE").as_deref() == Ok("abandon") {
        let mut ledger = accounting();
        let mut limits = ledger.limits().clone();
        limits.elapsed_ms = 120_000.try_into().unwrap();
        limits.decode.elapsed_ms = limits.elapsed_ms;
        ledger = Accounting::new(limits);
        drop(adapter.run(
            fixture::request(&binary, "cpu", &canary, false),
            &mut ledger,
        ));
        panic!("provisioner did not kill the owned supervisor");
    }
    fixture::descriptors(&canary);
    for (image, dynamic, mode, expected) in [
        (&binary, false, "deny", Ok(b"DENIED".to_vec())),
        (&dynamic, true, "deny", Ok(b"DENIED".to_vec())),
        (&dynamic, true, "loader", Ok(b"LOADER_DENIED".to_vec())),
        (&executable_memory, false, "wx", Ok(b"WX_DENIED".to_vec())),
        (&binary, false, "memory", Err(Refusal::Memory)),
        (&binary, false, "crash", Err(Refusal::Crash)),
        (&binary, false, "closure", Ok(b"CLOSURE_DENIED".to_vec())),
        (&binary, false, "flood", Err(Refusal::Output)),
        (&binary, false, "unadmitted", Err(Refusal::Output)),
        (&binary, false, "shape", Err(Refusal::Output)),
        (&binary, false, "extra", Err(Refusal::Output)),
        (&binary, false, "truncated", Err(Refusal::Output)),
    ] {
        let result = adapter.run(
            fixture::request(image, mode, &canary, dynamic),
            &mut accounting(),
        );
        assert_eq!(result, expected, "real containment effect {mode}");
        clean(&scratch, &group);
        println!("N17_EFFECT {mode}: expected result; tree/leaf/scratch absent");
    }
    let mut swapped = fixture::request(&binary, "deny", &canary, false);
    swapped.parser.digest = Digest::of(b"swapped launch bytes");
    assert_eq!(
        adapter.run(swapped, &mut accounting()),
        Err(Refusal::LaunchPin)
    );
    clean(&scratch, &group);
    for (mode, reason) in [
        ("decode", Failure::ExpandedBytes),
        ("entities", Failure::Entities),
    ] {
        let mut ledger = accounting();
        let mut limits = ledger.limits().clone();
        limits.decode.expanded_bytes = 100.try_into().unwrap();
        ledger.tighten(&limits);
        let result = adapter.run(fixture::request(&binary, mode, &canary, false), &mut ledger);
        assert!(
            matches!(result, Err(Refusal::Decode(ref refusal)) if refusal.reason == reason),
            "{result:?}"
        );
        clean(&scratch, &group);
    }
    process_limits(&adapter, &group, &binary, &canary, &scratch);
    live_and_active(&adapter, &group, &binary, &canary, &scratch);
    // A crash cannot poison unrelated documents or promote an earlier partial result.
    assert_eq!(
        adapter.run(
            fixture::request(&binary, "deny", &canary, false),
            &mut accounting()
        ),
        Ok(b"DENIED".to_vec())
    );
    clean(&scratch, &group);
    println!("N17_ACCEPTED kernel denial, W^X, resource/decode limits and owned cleanup");
}

/// Observe CPU throttling/deadline and PID-ceiling/owned-cancellation effects.
fn process_limits(
    adapter: &impl Isolation,
    group: &Path,
    binary: &Path,
    canary: &Path,
    scratch: &Path,
) {
    let mut ledger = accounting();
    let mut limits = ledger.limits().clone();
    limits.elapsed_ms = 1000.try_into().unwrap();
    ledger.tighten(&limits);
    let observed_group = group.to_owned();
    let observer = thread::spawn(move || {
        observe(
            &observed_group,
            "cpu.stat",
            "nr_throttled ",
            Instant::now() + Duration::from_secs(3),
        )
    });
    assert_eq!(
        adapter.run(fixture::request(binary, "cpu", canary, false), &mut ledger),
        Err(Refusal::Timeout)
    );
    assert!(
        observer.join().unwrap(),
        "CPU quota never throttled the hostile loop"
    );
    clean(scratch, group);
    let launch = fixture::request(binary, "fork", canary, false);
    let cancelled = Arc::clone(&launch.cancelled);
    let observed_group = group.to_owned();
    let observer = thread::spawn(move || {
        let denied = observe(
            &observed_group,
            "pids.events",
            "max ",
            Instant::now() + Duration::from_secs(4),
        );
        cancelled.store(true, Ordering::Release);
        denied
    });
    assert_eq!(
        adapter.run(launch, &mut accounting()),
        Err(Refusal::Cancelled)
    );
    assert!(
        observer.join().unwrap(),
        "forked descendants never reached the actual PID ceiling"
    );
    clean(scratch, group);
}

/// A live preparer is preserved; a committed active receipt refuses before launch.
fn live_and_active(
    adapter: &impl Isolation,
    group: &Path,
    binary: &Path,
    canary: &Path,
    scratch: &Path,
) {
    let preparing = scratch.join(".n17-live-preparing");
    fs::create_dir(&preparing).unwrap();
    let lock = File::open(&preparing).unwrap();
    lock.try_lock().unwrap();
    fs::write(preparing.join("cgroup-path"), "incomplete").unwrap();
    assert_eq!(
        adapter.run(
            fixture::request(binary, "deny", canary, false),
            &mut accounting()
        ),
        Ok(b"DENIED".to_vec())
    );
    assert!(preparing.is_dir());
    drop(lock);
    fs::remove_dir_all(preparing).unwrap();
    let active = scratch.join("n17-active");
    fs::create_dir(&active).unwrap();
    fs::write(active.join("cgroup-path"), group.to_str().unwrap()).unwrap();
    assert_eq!(
        adapter.run(
            fixture::request(binary, "deny", canary, false),
            &mut accounting()
        ),
        Err(Refusal::Cleanup)
    );
    assert!(active.is_dir());
    fs::remove_dir_all(active).unwrap();
    clean(scratch, group);
}
