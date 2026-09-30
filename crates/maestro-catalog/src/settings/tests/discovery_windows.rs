//! Real Windows ACL, unreadability and reparse-point probes, run on the CI host.
use super::discovery::Home;
use std::{fs, path::Path, process::Command};

/// Set a real ACL through the Windows host, with the file path supplied as data.
fn set_sddl(path: &Path, dacl: &str) {
    let script = concat!(
        "$ErrorActionPreference='Stop'; $a=Get-Acl -LiteralPath $env:PROBE_PATH; ",
        "$a.SetSecurityDescriptorSddlForm($env:PROBE_SDDL,",
        "[System.Security.AccessControl.AccessControlSections]::Access); ",
        "Set-Acl -LiteralPath $env:PROBE_PATH -AclObject $a",
    );
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("PROBE_PATH", path)
        .env("PROBE_SDDL", dacl)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn windows_owner_only_and_privileged_default_acl_shapes_accept_but_null_and_other_writers_refuse() {
    let home = Home::new();
    let root = home.plant(
        "project",
        "schema = 'maestro-preferences/1'\ntone = 'brief'",
    );
    let sid = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value",
        ])
        .output()
        .unwrap();
    assert!(sid.status.success());
    let sid = String::from_utf8(sid.stdout).unwrap();
    let owner = format!("D:P(A;;FA;;;{})", sid.trim());
    let directory = root.join(".maestro");
    let file = directory.join("config.toml");
    for target in [&directory, &file] {
        set_sddl(target, &owner);
    }
    assert!(home.load(Some(&root)).unwrap().discovery.file.is_some());
    let privileged = format!("{owner}(A;;FA;;;SY)(A;;FA;;;S-1-5-32-544)");
    for target in [&directory, &file] {
        set_sddl(target, &privileged);
    }
    assert!(home.load(Some(&root)).unwrap().discovery.file.is_some());
    for extra in ["(A;;GW;;;WD)", "(A;;GW;;;BU)"] {
        set_sddl(&file, &format!("{privileged}{extra}"));
        let snapshot = home.load(Some(&root)).unwrap();
        assert!(snapshot.discovery.file.is_none());
        assert!(snapshot.discovery.skipped[0].contains("other-writable"));
    }
    set_sddl(&file, "D:NO_ACCESS_CONTROL");
    let snapshot = home.load(Some(&root)).unwrap();
    assert!(snapshot.discovery.file.is_none());
    assert!(snapshot.discovery.skipped[0].contains("NULL DACL"));
    set_sddl(&file, &privileged);
}

#[test]
fn windows_unreadable_and_reparse_candidates_skip_to_safe_neighbor() {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let home = Home::new();
    let root = home.plant(
        "project",
        "schema = 'maestro-preferences/1'\ntone = 'brief'",
    );
    let nested = home.plant("project/nested", "invalid planted file");
    let file = nested.join(".maestro/config.toml");
    assert!(
        Command::new("icacls")
            .arg(&file)
            .args(["/deny", "*S-1-1-0:(R)"])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(home.load(Some(&nested)).unwrap().discovery.skipped.len(), 1);
    assert!(
        Command::new("icacls")
            .arg(&file)
            .args(["/remove:d", "*S-1-1-0"])
            .status()
            .unwrap()
            .success()
    );
    fs::remove_file(&file).unwrap();
    symlink_file(root.join(".maestro/config.toml"), &file).unwrap();
    assert_eq!(home.load(Some(&nested)).unwrap().discovery.skipped.len(), 1);
    fs::remove_file(&file).unwrap();
    fs::remove_dir(nested.join(".maestro")).unwrap();
    symlink_dir(root.join(".maestro"), nested.join(".maestro")).unwrap();
    let linked = home.load(Some(&nested)).unwrap();
    assert_eq!(linked.discovery.skipped.len(), 1);
    assert_eq!(
        linked.discovery.file,
        Some(root.join(".maestro/config.toml").canonicalize().unwrap())
    );
}
