//! Stage-two command construction is ordinary production code, not a host policy.
use super::bootstrap::handoff_command;
use maestro_test_scratch::disk_scratch_directory;
use std::{
    fs::{self, File},
    os::fd::AsRawFd as _,
    process::Stdio,
};

#[test]
fn n17_bootstrap_real_pinned_handoff_clears_environment_and_stderr() {
    let root = disk_scratch_directory().unwrap();
    fs::write(
        root.join("init"),
        concat!(
            "printf '%s\\n' \"$1\"\n/usr/bin/env\n",
            "printf 'untrusted stderr' >&2\nexit 0\n"
        ),
    )
    .unwrap();
    let image = File::open("/bin/sh").unwrap();
    let mut command = handoff_command(image.as_raw_fd(), "exact encoded configuration");
    assert_eq!(
        command.get_program(),
        format!("/proc/self/fd/{}", image.as_raw_fd()).as_str()
    );
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        ["init", "exact encoded configuration"]
    );
    let output = command
        .current_dir(&root)
        .stdout(Stdio::piped())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines[0], "exact encoded configuration");
    assert!(
        lines.len() == 2 && lines[1] == format!("PWD={}", root.display()),
        "stage two retained an ambient environment"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n17_bootstrap_pinned_descriptor_reopen_and_invalid_fd_refusal() {
    use super::{bootstrap::pinned_parser, port::Refusal};
    use std::io::Read as _;
    let root = disk_scratch_directory().unwrap();
    let path = root.join("image");
    fs::write(&path, "exact synthetic image").unwrap();
    let image = File::open(path).unwrap();
    let mut reopened = pinned_parser(image.as_raw_fd()).unwrap();
    let mut bytes = Vec::new();
    reopened.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"exact synthetic image");
    for descriptor in [-1, i32::MAX] {
        assert!(matches!(pinned_parser(descriptor), Err(Refusal::LaunchPin)));
    }
    fs::remove_dir_all(root).unwrap();
}
