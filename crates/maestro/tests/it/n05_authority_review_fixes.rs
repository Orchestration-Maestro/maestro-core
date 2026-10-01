//! Review regressions against real owner and unprivileged identities.
use super::{
    n05_authority_linux::{Fixture, grant},
    support::Ended,
};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{PermissionsExt as _, symlink},
    path::Path,
    process::Command,
};

#[test]
fn n05_stdout_only_launcher_cannot_qualify_or_write_receipt() {
    let fixture = Fixture::new();
    let launcher = fixture.home.tools().join("authority/launcher");
    fs::write(
        launcher,
        "#!/bin/sh\nprintf 'authority probe denied %s\\n' \"$1\"\n",
    )
    .unwrap();
    let result = fixture
        .home
        .run(&["authority", "qualify", "--config", &fixture.config]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(!Path::new(&fixture.store).join("qualification").exists());
}

/// Owner CLI request, using the exact path under inspection.
fn owner_request(fixture: &Fixture, path: &Path) -> Ended {
    fixture.home.run(&[
        "--json",
        "authority",
        "request",
        "--socket",
        &fixture.socket,
        "--authority-uid",
        &fixture.owner_uid,
        "--file",
        path.to_str().unwrap(),
    ])
}

#[test]
#[ignore = "needs sudo: UID 65534 replaces attacker-writable confirmation bytes"]
fn n05_needs_sudo_owner_refuses_rewritten_world_writable_request() {
    let fixture = Fixture::new();
    fixture.qualify();
    let _service = fixture.serve();
    let path = fixture.home.tools().join("authority/owner-request.json");
    fs::write(&path, "{}").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    let grant = grant();
    let value = json!({"action":"grant","grant":grant,"confirmation":grant});
    let rewritten = Command::new("sudo")
        .args([
            "-n",
            "setpriv",
            "--reuid",
            "65534",
            "--regid",
            "65534",
            "--clear-groups",
            "--",
            "sh",
            "-c",
            "printf '%s' \"$2\" > \"$1\"",
            "sh",
        ])
        .arg(&path)
        .arg(value.to_string())
        .output()
        .unwrap();
    assert!(rewritten.status.success(), "{rewritten:?}");
    let refused = owner_request(&fixture, &path);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let accepted = owner_request(&fixture, &path);
    assert_eq!(accepted.code, Some(0), "{accepted:?}");
}

#[test]
#[ignore = "needs sudo: owner refuses a symlink to a UID 65534-owned request"]
fn n05_needs_sudo_owner_refuses_symlink_confirmation() {
    let fixture = Fixture::new();
    fixture.qualify();
    let _service = fixture.serve();
    let target = fixture.home.tools().join("authority/untrusted.json");
    let grant = grant();
    fs::write(
        &target,
        json!({"action":"grant","grant":grant,"confirmation":grant}).to_string(),
    )
    .unwrap();
    let chown = Command::new("sudo")
        .args(["-n", "chown", "65534:65534"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(chown.status.success(), "{chown:?}");
    let path = fixture.home.tools().join("authority/linked.json");
    symlink(&target, &path).unwrap();
    let refused = owner_request(&fixture, &path);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    let direct = owner_request(&fixture, &target);
    assert_eq!(direct.code, Some(2), "{direct:?}");
}

#[test]
#[ignore = "needs sudo: owner CLI inspects a genuinely qualified authority"]
fn n05_needs_sudo_owner_cli_decide_inspects_current_permit() {
    let fixture = Fixture::new();
    fixture.qualify();
    let _service = fixture.serve();
    let grant = grant();
    assert_eq!(
        fixture.request(&json!({"action":"grant","grant":grant,"confirmation":grant}))["status"],
        "ok"
    );
    let decide =
        json!({"action":"decide","principal":"65534","operation":"fetch","target":grant["target"]});
    assert_eq!(fixture.request(&decide)["status"], "permit");
    let path = fixture.home.tools().join("authority/inspection.json");
    fs::write(&path, decide.to_string()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let accepted = owner_request(&fixture, &path);
    assert_eq!(accepted.code, Some(0), "{accepted:?}");
    assert!(accepted.stdout.contains("fixture-grant"), "{accepted:?}");
}

#[test]
fn n05_qualification_probe_budget_is_visible_and_bounded() {
    let fixture = Fixture::new();
    let help = fixture.home.run(&["authority", "qualify", "--help"]);
    assert!(help.stdout.contains("--probe-timeout-seconds"), "{help:?}");
    for value in ["0", "301"] {
        let refused = fixture.home.run(&[
            "authority",
            "qualify",
            "--config",
            &fixture.config,
            "--probe-timeout-seconds",
            value,
        ]);
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert!(!Path::new(&fixture.store).join("qualification").exists());
    }
}

#[test]
#[ignore = "needs sudo: qualification authenticates each of two attempted completions"]
fn n05_needs_sudo_qualification_refuses_second_connection() {
    let fixture = Fixture::new();
    let launcher = fixture.home.tools().join("authority/launcher");
    let original = fs::read_to_string(&launcher).unwrap();
    let repeated = original.replacen("exec sudo", "sudo", 1);
    fs::write(
        &launcher,
        format!(
            "{repeated}sudo -n setpriv --reuid=\"$uid\" --regid=\"$uid\" --clear-groups -- \"$@\"\n"
        ),
    )
    .unwrap();
    let refused = fixture
        .home
        .run(&["authority", "qualify", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(!Path::new(&fixture.store).join("qualification").exists());
    assert!(!Path::new(&fixture.socket).with_extension("probe").exists());
}

#[test]
#[ignore = "needs sudo: unchanged bytes alone cannot qualify a changed canary mode"]
fn n05_needs_sudo_qualification_checks_canary_mode() {
    let fixture = Fixture::new();
    let launcher = fixture.home.tools().join("authority/launcher");
    let original = fs::read_to_string(&launcher).unwrap();
    fs::write(
        &launcher,
        original.replacen("uid=$1", "chmod 640 \"$6/probe-witness\"\nuid=$1", 1),
    )
    .unwrap();
    let refused = fixture
        .home
        .run(&["authority", "qualify", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(!Path::new(&fixture.store).join("qualification").exists());
}

#[test]
fn n05_qualification_preserves_an_existing_endpoint() {
    let fixture = Fixture::new();
    let endpoint = Path::new(&fixture.socket).with_extension("probe");
    fs::write(&endpoint, "existing endpoint canary").unwrap();
    fs::write(
        fixture.home.tools().join("authority/launcher"),
        "#!/bin/sh\nprintf 'authority probe denied %s\\n' \"$1\"\n",
    )
    .unwrap();
    let refused = fixture
        .home
        .run(&["authority", "qualify", "--config", &fixture.config]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(
        fs::read_to_string(&endpoint).unwrap(),
        "existing endpoint canary"
    );
    assert!(!Path::new(&fixture.store).join("qualification").exists());
}
