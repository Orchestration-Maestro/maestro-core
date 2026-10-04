//! Host-provisioned public containment fixtures and explicit synthetic envelopes.
use super::n09_support::policy;
use maestro_acquisition::{
    isolation::{
        linux::{BootstrapMode, Host, Linux},
        port::{Launch, PinnedFile, RuntimeFile, ScopedRead},
    },
    transport::stream::Accounting,
};
use maestro_kernel::artifact::Digest;
use std::{
    env,
    fs::{self, File},
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::Duration,
};

/// A required env value has no fallback in a real qualification run.
pub(super) fn required(name: &str) -> PathBuf {
    PathBuf::from(env::var(name).unwrap())
}
/// Provisioned public binary opened once by the trusted fixture supervisor.
pub(super) fn pin(path: &Path) -> PinnedFile {
    PinnedFile {
        file: File::open(path).unwrap(),
        digest: Digest::of(&fs::read(path).unwrap()),
    }
}
/// Dedicated real systemd unit path, observed before manager-leaf placement.
pub(super) fn delegated() -> PathBuf {
    let group = fs::read_to_string("/proc/self/cgroup").unwrap();
    Path::new("/sys/fs/cgroup").join(
        group
            .strip_prefix("0::")
            .unwrap()
            .trim()
            .trim_start_matches('/'),
    )
}
/// No defaults: real tests require all exact fixture/host receipts.
pub(super) fn host(scratch: &Path, delegated: &Path) -> Linux {
    assert_eq!(
        env::var("MAESTRO_N17_REQUIRED").as_deref(),
        Ok("1"),
        "real containment qualification requires MAESTRO_N17_REQUIRED=1"
    );
    fs::create_dir_all(scratch).unwrap();
    fs::set_permissions(scratch, fs::Permissions::from_mode(0o700)).unwrap();
    Linux::new(Host {
        delegated: delegated.to_owned(),
        scratch: scratch.to_owned(),
        bootstrap: pin(&required("MAESTRO_N17_BOOTSTRAP")),
        bootstrap_mode: BootstrapMode::parse(Some(
            &env::var("MAESTRO_N17_BOOTSTRAP_MODE").unwrap(),
        ))
        .unwrap(),
        apparmor_required: env::var("MAESTRO_N17_PROFILE").as_deref() == Ok("required"),
    })
    .unwrap()
}
/// Test budgets are explicit tighter OA3 inputs, not production defaults.
pub(super) fn accounting() -> Accounting {
    let mut limits = policy().policy().sources.first().unwrap().limits.clone();
    limits.elapsed_ms = 5000.try_into().unwrap();
    limits.decode.elapsed_ms = 5000.try_into().unwrap();
    limits.memory_bytes = (64 * 1024 * 1024).try_into().unwrap();
    limits.decode.memory_bytes = limits.memory_bytes;
    limits.staging_bytes = (512 * 1024 * 1024).try_into().unwrap();
    limits.cpu_millicores = 100.try_into().unwrap();
    limits.decode.expanded_bytes = 1000.try_into().unwrap();
    limits.decode.nested_levels = 10.try_into().unwrap();
    limits.decode.members = 100.try_into().unwrap();
    let mut accounting = Accounting::new(limits);
    accounting.encoded(1000).unwrap();
    accounting
}
/// Static fixture has no hidden loader/library closure.
pub(super) fn request(binary: &Path, mode: &str, canary: &Path, dynamic: bool) -> Launch {
    let runtime = if dynamic { runtime(binary) } else { Vec::new() };
    let mut inputs = vec![ScopedRead {
        name: "document".into(),
        file: File::open(binary).unwrap(),
    }];
    if mode == "loader" {
        inputs.push(ScopedRead {
            name: "library".into(),
            file: File::open(required("MAESTRO_N17_LIBRARY")).unwrap(),
        });
    }
    Launch {
        parser: pin(binary),
        runtime,
        inputs,
        arguments: vec![mode.into(), canary.to_str().unwrap().into()],
        pids_max: 16,
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}
/// Resolve only the independently built public fixture's root-owned system files.
fn runtime(binary: &Path) -> Vec<RuntimeFile> {
    let output = Command::new("ldd").arg(binary).output().unwrap();
    assert!(output.status.success());
    let mut paths = Vec::new();
    for word in String::from_utf8(output.stdout).unwrap().split_whitespace() {
        if word.starts_with('/') && !paths.contains(&word.to_owned()) {
            paths.push(word.to_owned());
        }
    }
    paths
        .into_iter()
        .map(|path| RuntimeFile {
            executable: path == "/lib64/ld-linux-x86-64.so.2",
            path: path.trim_start_matches('/').into(),
            pinned: pin(Path::new(&path)),
        })
        .collect()
}
/// Verify leaf deletion, scratch deletion and absence of every tree task.
pub(super) fn clean(scratch: &Path, group: &Path) {
    assert_eq!(
        fs::read_dir(scratch).unwrap().count(),
        0,
        "owned scratch remains"
    );
    assert!(
        !fs::read_dir(group).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("n17-")),
        "owned worker leaf remains"
    );
    let prefix = group
        .strip_prefix("/sys/fs/cgroup")
        .unwrap()
        .to_string_lossy();
    for entry in fs::read_dir("/proc").unwrap() {
        let entry = entry.unwrap();
        let pid = entry.file_name().to_string_lossy().into_owned();
        super::n17_process_cleanup::reaped(
            &pid,
            || super::n17_process_cleanup::owned(&entry.path(), &format!("{prefix}/n17-")),
            || {
                // Same 5 ms polling cadence as the existing kernel-effect observer.
                thread::park_timeout(Duration::from_millis(5));
                Ok(())
            },
        )
        .unwrap();
    }
}

/// Hold a real kernel directory lock until the external provisioner sends SIGKILL.
pub(super) fn hold_preparing(scratch: &Path) -> ! {
    let preparing = scratch.join(".n17-sigkill-preparing");
    fs::create_dir(&preparing).unwrap();
    let lock = File::open(&preparing).unwrap();
    lock.try_lock().unwrap();
    fs::write(preparing.join("cgroup-path"), "incomplete").unwrap();
    fs::write(preparing.join("locked"), "held").unwrap();
    loop {
        thread::park();
    }
}

/// An actual unrelated inherited read descriptor must be closed by the bootstrap.
pub(super) fn descriptors(canary: &Path) {
    use rustix::io::{FdFlags, fcntl_getfd, fcntl_setfd};
    use std::os::fd::AsRawFd as _;
    let file = File::open(canary).unwrap();
    let flags = fcntl_getfd(&file).unwrap();
    fcntl_setfd(&file, FdFlags::empty()).unwrap();
    let output = Command::new(required("MAESTRO_N17_BOOTSTRAP"))
        .env_clear()
        .arg("probe")
        .arg(file.as_raw_fd().to_string())
        .output()
        .unwrap();
    fcntl_setfd(&file, flags).unwrap();
    assert!(
        output.status.success(),
        "unrelated descriptor survived bootstrap hygiene"
    );
    println!("N17_INHERITED_FD_CLOSED {}", file.as_raw_fd());
}
