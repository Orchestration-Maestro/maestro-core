//! Trusted single-threaded bootstrap, invoked only by the pinned launcher.
use super::{
    launch::Configuration,
    port::{BootstrapIo, Refusal},
    sandbox, syscalls,
};
use nix::{errno::Errno, fcntl::AtFlags, sched::CloneFlags, unistd::close};
use rustix::{
    process::{getgid, getuid},
    thread::no_new_privs,
};
use std::{
    ffi::CString,
    fs::{self, File},
    io::{self, BufRead, Read as _},
    net::{TcpListener, UdpSocket},
    os::fd::AsRawFd as _,
    path::Path,
    process::{Command, Stdio},
};

/// Existing launch barrier cap, unchanged from N17.
const CONFIG_BYTES: usize = 4 * 1024 * 1024;
/// All required non-user namespaces, applied after identity mapping.
const ISOLATED: CloneFlags = CloneFlags::CLONE_NEWNS
    .union(CloneFlags::CLONE_NEWPID)
    .union(CloneFlags::CLONE_NEWNET)
    .union(CloneFlags::CLONE_NEWIPC)
    .union(CloneFlags::CLONE_NEWUTS);

/// Select descriptors before closing the directory iterator itself.
pub(super) fn descriptors(
    entries: impl IntoIterator<Item = io::Result<String>>,
    keep: &[i32],
) -> Result<Vec<i32>, Refusal> {
    let mut descriptors = Vec::new();
    for entry in entries {
        let descriptor = entry
            .map_err(|_| Refusal::Containment)?
            .parse::<i32>()
            .map_err(|_| Refusal::Containment)?;
        if descriptor > 2 && !keep.contains(&descriptor) {
            descriptors.push(descriptor);
        }
    }
    Ok(descriptors)
}
/// EBADF is the completed directory iterator, not a failed closure of a live FD.
pub(super) fn close_result(result: Result<(), Errno>) -> Result<(), Refusal> {
    match result {
        Ok(()) | Err(Errno::EBADF) => Ok(()),
        Err(_) => Err(Refusal::Containment),
    }
}
/// Close unrelated inherited descriptors while proc still names this process.
pub(super) fn hygiene(keep: &[i32]) -> Result<(), Refusal> {
    let entries = fs::read_dir("/proc/self/fd").map_err(|_| Refusal::Containment)?;
    let descriptors = descriptors(
        entries.map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned())),
        keep,
    )?;
    for descriptor in descriptors {
        close_result(close(descriptor))?;
    }
    Ok(())
}
/// New user identity confers no host authority; every other namespace remains mandatory.
fn namespaces(effects: &impl BootstrapIo) -> Result<(), Refusal> {
    let uid = getuid().as_raw();
    let gid = getgid().as_raw();
    effects.unshare(CloneFlags::CLONE_NEWUSER)?;
    effects.map("/proc/self/setgroups", "deny")?;
    effects.map("/proc/self/uid_map", &format!("0 {uid} 1\n"))?;
    effects.map("/proc/self/gid_map", &format!("0 {gid} 1\n"))?;
    effects.unshare(ISOLATED)
}
/// The same PID-1 driver orders every policy before ready, then execs the pinned FD.
pub(super) fn init_with(
    config: &Configuration,
    output: &mut impl io::Write,
    effects: &impl BootstrapIo,
) -> Result<(), Refusal> {
    if effects.pid() != 1 {
        return Err(Refusal::Containment);
    }
    let parser = effects.parser(config.parser_fd)?;
    effects.hygiene(&[parser.as_raw_fd()])?;
    effects.filesystem(&config.root, config.memory_bytes)?;
    effects.landlock(Path::new("/"), config.interpreter.as_deref())?;
    effects.capabilities()?;
    let mut arguments = vec![CString::new("/parser").map_err(|_| Refusal::Configuration)?];
    for argument in &config.arguments {
        arguments.push(CString::new(argument.as_str()).map_err(|_| Refusal::Configuration)?);
    }
    effects.restrict()?;
    writeln!(output, "{{\"kind\":\"ready\"}}").map_err(|_| Refusal::Containment)?;
    output.flush().map_err(|_| Refusal::Containment)?;
    effects.exec(&parser, &arguments, &[], AtFlags::AT_EMPTY_PATH)
}
/// Explicit inputs keep environment capture out of the policy and bound both config routes.
pub(super) fn dispatch(
    args: impl Iterator<Item = String>,
    input: &mut impl BufRead,
    streams: (&mut impl io::Write, &mut impl io::Write),
    required_profile: bool,
    effects: &impl BootstrapIo,
) -> Result<(), Refusal> {
    let (output, diagnostics) = streams;
    let args: Vec<_> = args.take(5).collect();
    match args.as_slice() {
        [mode, root, canary] if mode == "probe-unprivileged" => {
            return unprivileged(Path::new(root), Path::new(canary), output, None);
        }
        [mode, root, canary, loader] if mode == "probe-unprivileged" => {
            return unprivileged(Path::new(root), Path::new(canary), output, Some(loader));
        }
        [mode, ..] if mode == "probe" && args.len() <= 2 => {
            effects.hygiene(&[])?;
            if let Some(descriptor) = args.get(1)
                && fs::read(format!("/proc/self/fd/{descriptor}")).is_ok()
            {
                return Err(Refusal::Containment);
            }
            let profile = effects.profile()?;
            writeln!(output, "N17_PROFILE_PROBE {}", profile.trim())
                .map_err(|_| Refusal::Containment)?;
            return namespaces(effects);
        }
        [mode, encoded] if mode == "init" => {
            if encoded.len() > CONFIG_BYTES {
                return Err(Refusal::Configuration);
            }
            let config = serde_json::from_str(encoded).map_err(|_| Refusal::Configuration)?;
            return init_with(&config, output, effects);
        }
        [] => {}
        _ => return Err(Refusal::Configuration),
    }
    let mut encoded = String::new();
    input
        .take((CONFIG_BYTES + 1) as u64)
        .read_line(&mut encoded)
        .map_err(|_| Refusal::Configuration)?;
    if encoded.len() > CONFIG_BYTES {
        return Err(Refusal::Configuration);
    }
    let config: Configuration =
        serde_json::from_str(&encoded).map_err(|_| Refusal::Configuration)?;
    effects.hygiene(&[config.bootstrap_fd, config.parser_fd])?;
    if required_profile {
        let profile = effects.profile()?;
        if !attached_profile(&profile) {
            return Err(Refusal::Unsupported);
        }
        writeln!(diagnostics, "N17_APPARMOR_ATTACHED {}", profile.trim())
            .map_err(|_| Refusal::Containment)?;
    }
    namespaces(effects)?;
    let mut command = handoff_command(
        config.bootstrap_fd,
        &serde_json::to_string(&config).map_err(|_| Refusal::Configuration)?,
    );
    if !effects.handoff(&mut command)? {
        return Err(Refusal::Crash);
    }
    Ok(())
}
/// Required production profile or the exact numeric provisioned-host scope, never complain.
fn attached_profile(profile: &str) -> bool {
    let profile = profile.trim_end_matches('\n');
    let Some(name) = profile
        .strip_suffix(" (enforce)")
        .or_else(|| profile.strip_suffix(" (unconfined)"))
    else {
        return false;
    };
    if name == "maestro-n17-parser-bootstrap" {
        return true;
    }
    let Some(scope) = name.strip_prefix("maestro-n17-parser-bootstrap-") else {
        return false;
    };
    let mut parts = scope.split('-');
    let numeric = |part: Option<&str>| {
        part.is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    };
    numeric(parts.next()) && numeric(parts.next()) && parts.next().is_none()
}

/// Stock-runner qualification of the actual Landlock/seccomp policy, with no namespaces.
fn unprivileged(
    root: &Path,
    canary: &Path,
    output: &mut impl io::Write,
    loader: Option<&str>,
) -> Result<(), Refusal> {
    hygiene(&[])?;
    sandbox::landlock(root, loader)?;
    qualify_files(
        no_new_privs().map_err(|_| Refusal::Unsupported)?,
        fs::read(canary).is_ok(),
        fs::write(root.join("input/document"), "modified").is_ok(),
        &fs::read(root.join("input/document")).map_err(|_| Refusal::Containment)?,
    )?;
    fs::write(root.join("work/output"), "scratch").map_err(|_| Refusal::Containment)?;
    syscalls::restrict()?;
    qualify_network(
        TcpListener::bind("127.0.0.1:0").is_ok(),
        UdpSocket::bind("127.0.0.1:0").is_ok(),
    )?;
    writeln!(
        output,
        "N17_UNPRIVILEGED_ACCEPTED strict V3, no_new_privs, scoped read/write, TCP/UDP denied"
    )
    .map_err(|_| Refusal::Containment)
}

/// Classify observed filesystem effects, independently testing every refusal reason.
pub(super) fn qualify_files(
    privileges: bool,
    canary_read: bool,
    input_write: bool,
    input: &[u8],
) -> Result<(), Refusal> {
    if !privileges || canary_read || input_write || input != b"scoped" {
        return Err(Refusal::Containment);
    }
    Ok(())
}
/// Both socket classes are independently mandatory denials.
pub(super) fn qualify_network(tcp: bool, udp: bool) -> Result<(), Refusal> {
    if tcp || udp {
        return Err(Refusal::Containment);
    }
    Ok(())
}

/// Only the explicit installed-profile requirement selects the attachment check.
pub(super) fn profile_required(value: Option<&str>) -> bool {
    value == Some("required")
}

/// Build stage two from the pinned FD, with no inherited environment or stderr.
pub(super) fn handoff_command(descriptor: i32, encoded: &str) -> Command {
    let mut command = Command::new(format!("/proc/self/fd/{descriptor}"));
    command
        .arg("init")
        .arg(encoded)
        .env_clear()
        .stderr(Stdio::null());
    command
}

/// Reopen only the supplied pinned FD before proc is hidden, never its original path.
pub(super) fn pinned_parser(descriptor: i32) -> Result<File, Refusal> {
    File::open(format!("/proc/self/fd/{descriptor}")).map_err(|_| Refusal::LaunchPin)
}
