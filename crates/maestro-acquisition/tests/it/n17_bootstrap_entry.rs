//! Exercise the actual binary main, including the non-Linux refusal.
use std::{
    env,
    io::Write as _,
    process::{Command, Stdio},
};

#[test]
fn n17_actual_bootstrap_binary_invalid_input_refuses() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro-parser-bootstrap"));
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = command.spawn().unwrap();
    let written = child
        .stdin
        .take()
        .unwrap()
        .write_all(b"not configuration\n");
    #[cfg(target_os = "linux")]
    written.unwrap();
    // Other platforms refuse before consuming stdin and may close the pipe first.
    #[cfg(not(target_os = "linux"))]
    drop(written);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success(), "{output:?}");
    #[cfg(target_os = "linux")]
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Error: Configuration\n"
    );
    #[cfg(not(target_os = "linux"))]
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Linux parser containment unsupported")
    );
}
