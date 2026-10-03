//! Real kernel qualification is opt-in, with required host setup, never a skip.
use maestro_acquisition::isolation::linux::{BootstrapMode, Host, Linux};
use maestro_acquisition::isolation::port::{PinnedFile, Refusal};
use maestro_kernel::artifact::Digest;
use maestro_test_scratch::scratch_directory;
use std::{
    fs::{self, File},
    os::unix::fs::PermissionsExt as _,
    process::Command,
};

#[test]
fn n17_missing_unknown_mode_and_unsupported_host_refuse() {
    for value in [None, Some(""), Some("auto"), Some("unknown")] {
        assert_eq!(BootstrapMode::parse(value), Err(Refusal::Configuration));
    }
    assert_eq!(
        BootstrapMode::parse(Some("sealed")),
        Ok(BootstrapMode::Sealed)
    );
    assert_eq!(
        BootstrapMode::parse(Some("installed")),
        Ok(BootstrapMode::Installed)
    );
    let scratch = scratch_directory().unwrap();
    fs::create_dir_all(&scratch).unwrap();
    fs::set_permissions(&scratch, fs::Permissions::from_mode(0o700)).unwrap();
    let binary = scratch.join("binary");
    fs::write(&binary, "synthetic").unwrap();
    let host = Host {
        delegated: scratch.join("not-a-systemd-unit"),
        scratch: scratch.clone(),
        bootstrap: PinnedFile {
            file: File::open(&binary).unwrap(),
            digest: Digest::of(b"synthetic"),
        },
        bootstrap_mode: BootstrapMode::Sealed,
        apparmor_required: false,
    };
    assert!(matches!(Linux::new(host), Err(Refusal::Configuration)));
    assert!(!scratch.join("not-a-systemd-unit").exists());
    fs::remove_dir_all(scratch).unwrap();
}

/// These unprivileged kernel layers require no delegated host or user namespace setup.
#[test]
#[cfg(target_arch = "x86_64")]
fn n17_default_unprivileged_landlock_seccomp_and_no_new_privs() {
    run_probe(None);
    run_probe(Some("loader"));
}
/// Disposable real-kernel probe with both loader/no-loader policies.
#[cfg(target_arch = "x86_64")]
fn run_probe(loader: Option<&str>) {
    let scratch = scratch_directory().unwrap();
    let root = scratch.join("view");
    fs::create_dir_all(root.join("input")).unwrap();
    fs::create_dir(root.join("work")).unwrap();
    fs::write(root.join("input/document"), "scoped").unwrap();
    // A probe must succeed with non-executable data: this CLI cannot execute files.
    fs::write(root.join("parser"), "pinned fixture").unwrap();
    fs::write(root.join("loader"), "synthetic loader").unwrap();
    let canary = scratch.join("host-canary");
    fs::write(&canary, "host-only synthetic").unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro-parser-bootstrap"));
    command
        .env_clear()
        .arg("probe-unprivileged")
        .arg(&root)
        .arg(&canary);
    if let Some(loader) = loader {
        command.arg(loader);
    }
    let profile_parent = super::n17_child_profiles::configure(&mut command, &root);
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("N17_UNPRIVILEGED_ACCEPTED ")
    );
    assert_eq!(
        fs::read_to_string(root.join("input/document")).unwrap(),
        "scoped"
    );
    assert_eq!(
        fs::read_to_string(root.join("work/output")).unwrap(),
        "scratch"
    );
    assert_eq!(fs::read_to_string(canary).unwrap(), "host-only synthetic");
    super::n17_child_profiles::collect(&root, profile_parent.as_deref());
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn n17_default_child_closes_actual_inherited_descriptor_before_probe() {
    use rustix::io::{FdFlags, fcntl_getfd, fcntl_setfd};
    use std::{env, os::fd::AsRawFd as _};
    let scratch = scratch_directory().unwrap();
    fs::create_dir_all(&scratch).unwrap();
    let canary = scratch.join("canary");
    fs::write(&canary, "synthetic").unwrap();
    let file = File::open(canary).unwrap();
    let flags = fcntl_getfd(&file).unwrap();
    fcntl_setfd(&file, FdFlags::empty()).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro-parser-bootstrap"));
    command
        .env_clear()
        .arg("probe")
        .arg(file.as_raw_fd().to_string());
    if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let output = command.output().unwrap();
    fcntl_setfd(&file, flags).unwrap();
    // Namespace creation may refuse on this host, but only after hygiene/profile reporting.
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("N17_PROFILE_PROBE ")
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn n17_actual_init_binary_refuses_non_pid_one_before_parser_or_policy() {
    use std::env;
    let encoded = concat!(
        r#"{"root":"/synthetic","parser_fd":-1,"bootstrap_fd":-1,"#,
        r#""arguments":[],"interpreter":null,"memory_bytes":123}"#
    );
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro-parser-bootstrap"));
    command.env_clear().arg("init").arg(encoded);
    if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Error: Containment\n"
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn n17_production_probe_refuses_execute_arguments() {
    use std::env;
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro-parser-bootstrap"));
    command.env_clear().args([
        "probe-unprivileged",
        "/synthetic",
        "/synthetic",
        "--exec",
        "/synthetic/parser",
    ]);
    if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Error: Configuration\n"
    );
    assert!(output.stdout.is_empty());
}
