//! Observe actual process retirement without treating a dead proc entry as a live task.
use maestro_test_scratch::scratch_directory;
use std::{fs, io, path::Path, process, time::Instant};

/// Kernel status fields needed to distinguish live tasks from pending reaping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Status {
    state: char,
    parent: u32,
}

/// Parse the kernel's status, refusing missing or malformed identity fields.
pub(super) fn status(text: &str) -> io::Result<Status> {
    let state = text.lines().find_map(|line| line.strip_prefix("State:"));
    let parent = text.lines().find_map(|line| line.strip_prefix("PPid:"));
    let state = state
        .and_then(|value| value.split_whitespace().next())
        .filter(|value| value.len() == 1)
        .and_then(|value| value.chars().next());
    let parent = parent.and_then(|value| value.trim().parse().ok());
    match (state, parent) {
        (Some(state), Some(parent)) => Ok(Status { state, parent }),
        _ => Err(io::ErrorKind::InvalidData.into()),
    }
}

/// Recheck ownership on every observation, including PID disappearance/reuse races.
pub(super) fn owned(path: &Path, prefix: &str) -> io::Result<Option<Status>> {
    let Ok(group) = fs::read_to_string(path.join("cgroup")) else {
        // As before, inaccessible/unrelated proc entries are not attributed to this scope.
        return Ok(None);
    };
    if !group.contains(prefix) {
        return Ok(None);
    }
    match fs::read_to_string(path.join("status")) {
        Ok(text) => status(&text).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// No internal deadline: the already-required 120-second unit watchdog bounds retirement.
pub(super) fn reaped(
    pid: &str,
    mut observe: impl FnMut() -> io::Result<Option<Status>>,
    mut wait: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let started = Instant::now();
    let mut previous = None;
    while let Some(status) = observe()? {
        if status.state != 'Z' && status.state != 'X' {
            return Err(io::Error::other(format!(
                "owned process remains: pid={pid} state={} PPid={}",
                status.state, status.parent
            )));
        }
        if previous != Some(status) {
            println!(
                "N17_REAP_WAIT pid={pid} state={} PPid={} elapsed_ms={} {}",
                status.state,
                status.parent,
                started.elapsed().as_millis(),
                parent(status.parent)
            );
            previous = Some(status);
        }
        if let Err(error) = wait() {
            println!(
                "N17_REAP_INTERRUPTED pid={pid} state={} PPid={} elapsed_ms={} error={error}",
                status.state,
                status.parent,
                started.elapsed().as_millis()
            );
            return Err(error);
        }
    }
    if let Some(status) = previous {
        println!(
            "N17_REAP_ABSENT pid={pid} state={} PPid={} elapsed_ms={}",
            status.state,
            status.parent,
            started.elapsed().as_millis()
        );
    }
    Ok(())
}

/// Retain parent liveness/name/image so a never-reaped zombie is diagnosable, not excused.
fn parent(pid: u32) -> String {
    let text = fs::read_to_string(format!("/proc/{pid}/status"));
    let executable = fs::read_link(format!("/proc/{pid}/exe"));
    match text {
        Ok(text) => format!(
            "parent_state={:?} parent_name={:?} parent_exe={executable:?}",
            status(&text).map(|status| status.state),
            text.lines().find_map(|line| line.strip_prefix("Name:"))
        ),
        Err(error) => format!("parent_status_unreadable={error} parent_exe={executable:?}"),
    }
}

#[test]
fn n17_proc_ownership_disappearance_and_status_errors_use_actual_scoped_files() {
    let root = scratch_directory().unwrap();
    fs::create_dir_all(&root).unwrap();
    let prefix = "/synthetic/unit/n17-";
    assert_eq!(owned(&root, prefix).unwrap(), None);
    fs::write(root.join("cgroup"), "0::/unrelated\n").unwrap();
    assert_eq!(owned(&root, prefix).unwrap(), None);
    fs::write(
        root.join("cgroup"),
        "0::/synthetic/unit/n17-owned (deleted)\n",
    )
    .unwrap();
    assert_eq!(owned(&root, prefix).unwrap(), None);
    fs::write(root.join("status"), "State: Z (zombie)\nPPid: 0\n").unwrap();
    assert_eq!(
        owned(&root, prefix).unwrap(),
        Some(Status {
            state: 'Z',
            parent: 0
        })
    );
    fs::write(root.join("status"), "malformed").unwrap();
    assert_eq!(
        owned(&root, prefix).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    fs::remove_file(root.join("status")).unwrap();
    fs::create_dir(root.join("status")).unwrap();
    assert!(owned(&root, prefix).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n17_proc_parent_diagnostics_record_actual_unprivileged_process_identity() {
    let report = parent(process::id());
    assert!(report.contains("parent_state=Ok("), "{report}");
    assert!(report.contains("parent_name=Some("), "{report}");
    assert!(report.contains("parent_exe=Ok("), "{report}");
}

#[test]
fn n17_proc_status_parser_keeps_state_and_parent_and_refuses_missing_fields() {
    for state in ['Z', 'X', 'R', 'S', 'D', 'T', 'I'] {
        let text = format!("Name:\tsynthetic\nState:\t{state} (description)\nPPid:\t123\n");
        assert_eq!(status(&text).unwrap(), Status { state, parent: 123 });
    }
    for text in [
        "",
        "State: Z",
        "PPid: 1",
        "State: ZZ\nPPid: 1",
        "State: Z\nPPid: -1",
    ] {
        assert_eq!(status(text).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}

#[test]
fn n17_dead_proc_entry_waits_until_absent_without_weakening_absence() {
    for state in ['Z', 'X'] {
        let mut observations = [Some(Status { state, parent: 0 }), None].into_iter();
        let mut waits = 0;
        reaped(
            "synthetic",
            || Ok(observations.next().unwrap()),
            || {
                waits += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(waits, 1);
        assert!(observations.next().is_none());
    }
}

#[test]
fn n17_live_proc_entry_refuses_immediately_in_every_non_dead_state() {
    for state in ['R', 'S', 'D', 'T', 't', 'I', 'W', '?'] {
        let error = reaped(
            "synthetic",
            || Ok(Some(Status { state, parent: 0 })),
            || panic!("a live process must never wait"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("owned process remains"));
    }
}

#[test]
fn n17_dead_proc_entry_watchdog_expiry_and_live_transition_fail() {
    let error = reaped(
        "synthetic",
        || {
            Ok(Some(Status {
                state: 'Z',
                parent: 0,
            }))
        },
        || Err(io::ErrorKind::TimedOut.into()),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    let mut observations = [
        Some(Status {
            state: 'Z',
            parent: 0,
        }),
        Some(Status {
            state: 'R',
            parent: 0,
        }),
    ]
    .into_iter();
    let error = reaped("synthetic", || Ok(observations.next().unwrap()), || Ok(())).unwrap_err();
    assert!(error.to_string().contains("state=R"));
}
