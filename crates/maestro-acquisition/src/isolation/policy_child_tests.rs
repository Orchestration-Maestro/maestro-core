//! Execution proofs live only in disposable test children, never a production CLI.
use super::{sandbox, syscalls};
use maestro_test_scratch::disk_scratch_directory;
use std::{
    env, fs,
    io::{self, copy},
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::Command,
};

/// The worker runs the actual policy, not a copy or an unprivileged production route.
fn worker(root: &Path, loader: bool) {
    assert_eq!(
        sandbox::landlock(root, if loader { Some("loader") } else { None }),
        Ok(()),
        "required hard Landlock V3 unavailable in disposable child"
    );
    fs::read_dir(root).unwrap();
    fs::read(root.join("input/document")).unwrap();
    assert_eq!(syscalls::restrict(), Ok(()), "required seccomp unavailable");
    let status = Command::new(root.join("parser"))
        .env_clear()
        .status()
        .unwrap();
    assert!(status.success(), "allowed parser exec refused: {status:?}");
    let interpreter = Command::new(root.join("loader")).env_clear().status();
    if loader {
        assert!(
            interpreter.unwrap().success(),
            "allowed interpreter exec refused"
        );
    } else {
        assert!(
            interpreter.is_err_and(|error| error.kind() == io::ErrorKind::PermissionDenied),
            "unselected interpreter received Execute rights"
        );
    }
    let denied = Command::new(root.join("work/program")).env_clear().status();
    assert!(
        denied.is_err_and(|error| error.kind() == io::ErrorKind::PermissionDenied),
        "work execution was not denied by the production policy"
    );
    fs::write(root.join("work/after-filter"), "allowed").unwrap();
}
#[test]
fn n17_default_child_parser_loader_and_work_execute_rights() {
    if let Some(root) = env::var_os("MAESTRO_N17_POLICY_CHILD") {
        worker(
            Path::new(&root),
            env::var("MAESTRO_N17_POLICY_LOADER").as_deref() == Ok("1"),
        );
        return;
    }
    for loader in [false, true] {
        let root = disk_scratch_directory().unwrap();
        fs::create_dir(root.join("work")).unwrap();
        fs::create_dir(root.join("input")).unwrap();
        fs::write(root.join("input/document"), "scoped").unwrap();
        for program in ["parser", "loader", "work/program"] {
            executable(&root.join(program));
        }
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .env_clear()
            .env("MAESTRO_N17_POLICY_CHILD", &root)
            .env("MAESTRO_N17_POLICY_LOADER", if loader { "1" } else { "0" })
            .args([
                "--exact",
                concat!(
                    "isolation::policy_child_tests::",
                    "n17_default_child_parser_loader_and_work_execute_rights"
                ),
                "--nocapture",
            ]);
        let parent_profile = env::var_os("LLVM_PROFILE_FILE").map(PathBuf::from);
        if parent_profile.is_some() {
            command.env("LLVM_PROFILE_FILE", root.join("work/policy-%p-%m.profraw"));
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            fs::read(root.join("work/after-filter")).unwrap(),
            b"allowed"
        );
        if let Some(destination) = parent_profile {
            collect(&root, destination.parent().unwrap());
        }
        fs::remove_dir_all(root).unwrap();
    }
}

/// Only fresh regular profiles from this test-owned child may enter LLVM's directory.
fn collect(root: &Path, destination: &Path) {
    let profiles: Vec<_> = fs::read_dir(root.join("work"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "profraw")
        })
        .collect();
    assert!(
        !profiles.is_empty(),
        "disposable policy child produced no real LLVM profile"
    );
    for profile in profiles {
        assert!(profile.file_type().unwrap().is_file());
        let name = format!(
            "{}-{}",
            root.file_name().unwrap().to_string_lossy(),
            profile.file_name().to_string_lossy()
        );
        let mut target = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination.join(name))
            .unwrap();
        let mut source = fs::File::open(profile.path()).unwrap();
        assert!(copy(&mut source, &mut target).unwrap() > 0);
    }
}

/// No compiler, runtime libraries, vendor binary or dynamic loader is needed.
fn executable(path: &Path) {
    let mut image = vec![0_u8; 132];
    image[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    image[16..18].copy_from_slice(&2_u16.to_le_bytes());
    image[18..20].copy_from_slice(&62_u16.to_le_bytes());
    image[20..24].copy_from_slice(&1_u32.to_le_bytes());
    image[24..32].copy_from_slice(&0x40_0078_u64.to_le_bytes());
    image[32..40].copy_from_slice(&64_u64.to_le_bytes());
    image[52..54].copy_from_slice(&64_u16.to_le_bytes());
    image[54..56].copy_from_slice(&56_u16.to_le_bytes());
    image[56..58].copy_from_slice(&1_u16.to_le_bytes());
    image[64..68].copy_from_slice(&1_u32.to_le_bytes());
    image[68..72].copy_from_slice(&5_u32.to_le_bytes());
    image[80..88].copy_from_slice(&0x40_0000_u64.to_le_bytes());
    image[96..104].copy_from_slice(&132_u64.to_le_bytes());
    image[104..112].copy_from_slice(&132_u64.to_le_bytes());
    image[112..120].copy_from_slice(&4096_u64.to_le_bytes());
    // mov eax,60; xor edi,edi; syscall; padding (normal Linux process exit).
    image[120..].copy_from_slice(&[0xb8, 60, 0, 0, 0, 0x31, 0xff, 0x0f, 0x05, 0, 0, 0]);
    fs::write(path, image).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o555)).unwrap();
}
