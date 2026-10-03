//! Trusted single-threaded bootstrap, invoked only by the pinned launcher.
use super::{launch::Configuration, port::Refusal, sandbox, syscalls};
use nix::{
    errno::Errno,
    fcntl::AtFlags,
    sched::{CloneFlags, unshare},
    unistd::{close, execveat},
};
use rustix::{
    process::{getgid, getpid, getuid},
    thread::no_new_privs,
};
use std::{
    env,
    ffi::CString,
    fs::{self, File},
    io::{self, BufRead as _, Read as _},
    net::{TcpListener, UdpSocket},
    os::fd::AsRawFd as _,
    path::Path,
    process::{Command, Stdio},
};

/// Close unrelated inherited descriptors while `/proc` still names this process.
fn hygiene(keep: &[i32]) -> Result<(), Refusal> {
    let entries = fs::read_dir("/proc/self/fd").map_err(|_| Refusal::Containment)?;
    let mut descriptors = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| Refusal::Containment)?;
        let descriptor = entry
            .file_name()
            .to_string_lossy()
            .parse::<i32>()
            .map_err(|_| Refusal::Containment)?;
        if descriptor > 2 && !keep.contains(&descriptor) {
            descriptors.push(descriptor);
        }
    }
    for descriptor in descriptors {
        // The completed directory iterator itself is already closed.
        if let Err(error) = close(descriptor)
            && error != Errno::EBADF
        {
            return Err(Refusal::Containment);
        }
    }
    Ok(())
}
/// New user identity confers no host/grant authority; all other namespaces are mandatory.
fn namespaces() -> Result<(), Refusal> {
    let uid = getuid().as_raw();
    let gid = getgid().as_raw();
    let [user, others] = namespace_plan();
    unshare(user).map_err(|_| Refusal::Unsupported)?;
    fs::write("/proc/self/setgroups", "deny").map_err(|_| Refusal::Unsupported)?;
    fs::write("/proc/self/uid_map", format!("0 {uid} 1\n")).map_err(|_| Refusal::Unsupported)?;
    fs::write("/proc/self/gid_map", format!("0 {gid} 1\n")).map_err(|_| Refusal::Unsupported)?;
    unshare(others).map_err(|_| Refusal::Unsupported)
}
/// Stage two is PID 1. Its death makes the kernel kill every namespace descendant.
fn init(config: &Configuration, output: &mut impl io::Write) -> Result<(), Refusal> {
    if getpid().as_raw_nonzero().get() != 1 {
        return Err(Refusal::Containment);
    }
    let parser = File::open(format!("/proc/self/fd/{}", config.parser_fd))
        .map_err(|_| Refusal::LaunchPin)?;
    hygiene(&[parser.as_raw_fd()])?;
    sandbox::filesystem(config)?;
    sandbox::landlock(Path::new("/"), config.interpreter.as_deref())?;
    sandbox::capabilities()?;
    let mut arguments = vec![CString::new("/parser").map_err(|_| Refusal::Configuration)?];
    for argument in &config.arguments {
        arguments.push(CString::new(argument.as_str()).map_err(|_| Refusal::Configuration)?);
    }
    syscalls::restrict()?;
    writeln!(output, "{{\"kind\":\"ready\"}}").map_err(|_| Refusal::Containment)?;
    output.flush().map_err(|_| Refusal::Containment)?;
    execveat(
        &parser,
        c"",
        &arguments,
        &[] as &[CString],
        AtFlags::AT_EMPTY_PATH,
    )
    .map_err(|_| Refusal::Containment)?;
    Ok(())
}
/// Entrypoint must run in a separate, single-threaded executable, never in core.
/// # Errors
/// Every namespace, hard Landlock, seccomp or pinned-image failure refuses.
pub fn run(output: &mut impl io::Write, diagnostics: &mut impl io::Write) -> Result<(), Refusal> {
    if env::args().nth(1).as_deref() == Some("probe-unprivileged") {
        let root = env::args().nth(2).ok_or(Refusal::Configuration)?;
        let canary = env::args().nth(3).ok_or(Refusal::Configuration)?;
        return unprivileged(Path::new(&root), Path::new(&canary), output);
    }
    if env::args().nth(1).as_deref() == Some("probe") {
        hygiene(&[])?;
        if let Some(descriptor) = env::args().nth(2)
            && fs::read(format!("/proc/self/fd/{descriptor}")).is_ok()
        {
            return Err(Refusal::Containment);
        }
        let profile =
            fs::read_to_string("/proc/self/attr/current").map_err(|_| Refusal::Unsupported)?;
        writeln!(output, "N17_PROFILE_PROBE {}", profile.trim())
            .map_err(|_| Refusal::Containment)?;
        namespaces()?;
        return Ok(());
    }
    if env::args().nth(1).as_deref() == Some("init") {
        let encoded = env::args().nth(2).ok_or(Refusal::Configuration)?;
        return init(
            &serde_json::from_str(&encoded).map_err(|_| Refusal::Configuration)?,
            output,
        );
    }
    let mut encoded = String::new();
    io::stdin()
        .lock()
        .take(4 * 1024 * 1024 + 1)
        .read_line(&mut encoded)
        .map_err(|_| Refusal::Configuration)?;
    if encoded.len() > 4 * 1024 * 1024 {
        return Err(Refusal::Configuration);
    }
    let config: Configuration =
        serde_json::from_str(&encoded).map_err(|_| Refusal::Configuration)?;
    hygiene(&[config.bootstrap_fd, config.parser_fd])?;
    // Installed-mode CI verifies this exact exec attached the launcher-only profile.
    if env::var("MAESTRO_N17_PROFILE").as_deref() == Ok("required") {
        let profile =
            fs::read_to_string("/proc/self/attr/current").map_err(|_| Refusal::Unsupported)?;
        if !profile.starts_with("maestro-n17-parser-bootstrap ") {
            return Err(Refusal::Unsupported);
        }
        writeln!(diagnostics, "N17_APPARMOR_ATTACHED {}", profile.trim())
            .map_err(|_| Refusal::Containment)?;
    }
    namespaces()?;
    let status = Command::new(format!("/proc/self/fd/{}", config.bootstrap_fd))
        .arg("init")
        .arg(serde_json::to_string(&config).map_err(|_| Refusal::Configuration)?)
        .env_clear()
        .stderr(Stdio::null())
        .status()
        .map_err(|_| Refusal::Containment)?;
    if !status.success() {
        return Err(Refusal::Crash);
    }
    Ok(())
}

/// Namespaces as explicit data; UID/GID mapping separates the two stages.
fn namespace_plan() -> [CloneFlags; 2] {
    [
        CloneFlags::CLONE_NEWUSER,
        CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWUTS,
    ]
}
/// Stock-runner qualification of the actual Landlock/seccomp policy, with no namespaces.
fn unprivileged(root: &Path, canary: &Path, output: &mut impl io::Write) -> Result<(), Refusal> {
    hygiene(&[])?;
    sandbox::landlock(root, None)?;
    if !no_new_privs().map_err(|_| Refusal::Unsupported)?
        || fs::read(canary).is_ok()
        || fs::write(root.join("input/document"), "modified").is_ok()
        || fs::read(root.join("input/document")).map_err(|_| Refusal::Containment)? != b"scoped"
    {
        return Err(Refusal::Containment);
    }
    fs::write(root.join("work/output"), "scratch").map_err(|_| Refusal::Containment)?;
    syscalls::restrict()?;
    if TcpListener::bind("127.0.0.1:0").is_ok() || UdpSocket::bind("127.0.0.1:0").is_ok() {
        return Err(Refusal::Containment);
    }
    writeln!(
        output,
        "N17_UNPRIVILEGED_ACCEPTED strict V3, no_new_privs, scoped read/write, TCP/UDP denied"
    )
    .map_err(|_| Refusal::Containment)
}

#[cfg(test)]
mod tests {
    use super::namespace_plan;
    #[test]
    fn n17_default_namespace_plan_has_every_required_namespace() {
        let [user, others] = namespace_plan();
        assert_eq!(user.bits(), 0x1000_0000);
        assert_eq!(others.bits(), 0x6c02_0000);
    }
}
