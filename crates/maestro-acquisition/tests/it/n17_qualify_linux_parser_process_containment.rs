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
    let scratch = scratch_directory().unwrap();
    let root = scratch.join("view");
    fs::create_dir_all(root.join("input")).unwrap();
    fs::create_dir(root.join("work")).unwrap();
    fs::write(root.join("input/document"), "scoped").unwrap();
    fs::write(root.join("parser"), "pinned fixture").unwrap();
    let canary = scratch.join("host-canary");
    fs::write(&canary, "host-only synthetic").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_maestro-parser-bootstrap"))
        .env_clear()
        .arg("probe-unprivileged")
        .arg(&root)
        .arg(&canary)
        .output()
        .unwrap();
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
    fs::remove_dir_all(scratch).unwrap();
}
