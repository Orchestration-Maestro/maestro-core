//! Every process pins the same safe preference snapshot before effects.
use super::support::{Home, initialize_mcp};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A configured project inside the disposable home.
fn project(home: &Home, body: &str) -> PathBuf {
    let root = home.root().join("project");
    fs::create_dir_all(root.join(".maestro")).unwrap();
    fs::write(root.join(".maestro/config.toml"), body).unwrap();
    root
}

#[test]
fn malformed_selected_preferences_refuse_before_status_or_journal_effects() {
    let home = Home::bare();
    let root = project(&home, "schema = 'maestro-preferences/1'\nunknown = true\n");
    for args in [vec!["status"], vec!["config", "history"]] {
        let result = home.run_in(&root, &args);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(result.stderr.contains("unknown"), "{result:?}");
    }
    assert!(!home.data().join("kernel.sqlite3").exists());
}

#[test]
fn nearest_file_replaces_ancestors_and_restart_observes_edits() {
    let home = Home::bare();
    let root = project(&home, "schema = 'maestro-preferences/1'\ntone = 'brief'\n");
    fs::create_dir_all(root.join("nested/.maestro")).unwrap();
    let file = root.join("nested/.maestro/config.toml");
    fs::write(&file, "schema = 'maestro-preferences/1'\nlanguage = 'ja'\n").unwrap();
    let nested = root.join("nested");
    let result = home.run_in(&nested, &["--json", "config", "get", "tone"]);
    assert_eq!(result.code, Some(0), "{result:?}");
    assert_eq!(result.json()["value"], "normal");
    assert_eq!(result.json()["source"], json!({"layer": "default"}));
    fs::write(
        &file,
        "schema = 'maestro-preferences/1'\ntone = 'detailed'\n",
    )
    .unwrap();
    let restarted = home.run_in(&nested, &["--json", "config", "get", "tone"]);
    assert_eq!(restarted.code, Some(0), "{restarted:?}");
    assert_eq!(restarted.json()["value"], "detailed");
}

#[test]
fn explicit_mcp_workspace_outside_home_warns_and_falls_back_without_parsing() {
    let home = Home::bare();
    let outside = Home::bare();
    let root = project(&outside, "not valid preferences");
    let (child, mut input) = home.start_with_stdin(&[
        "--set",
        "models.compute=off",
        "mcp",
        "--workspace",
        root.to_str().unwrap(),
    ]);
    initialize_mcp(&mut input);
    drop(input);
    let result = child.finish();
    assert_eq!(result.code, Some(0), "{result:?}");
    assert!(result.stderr.contains("maestro trust add"), "{result:?}");
}

#[test]
fn other_writable_invalid_candidate_is_skipped_for_safe_parent() {
    let home = Home::bare();
    let root = project(&home, "schema = 'maestro-preferences/1'\ntone = 'brief'\n");
    let nested = root.join("nested");
    fs::create_dir_all(nested.join(".maestro")).unwrap();
    let file = nested.join(".maestro/config.toml");
    fs::write(&file, "not valid preferences").unwrap();
    make_other_writable(&file);
    let result = home.run_in(&nested, &["--json", "config", "get", "tone"]);
    assert_eq!(result.code, Some(0), "{result:?}");
    assert_eq!(result.json()["value"], "brief");
    assert!(result.stderr.contains("skipped"), "{result:?}");
}

#[test]
fn mcp_reports_workspace_or_user_fallback_without_paths() {
    let home = Home::bare();
    let root = project(&home, "schema = 'maestro-preferences/1'\ntone = 'brief'\n");
    fs::write(
        home.config().join("preferences.toml"),
        "schema = 'maestro-preferences/1'\nlanguage = 'ja'\n",
    )
    .unwrap();
    let empty = home.root().join("empty-workspace");
    fs::create_dir(&empty).unwrap();
    for user_present in [true, false] {
        if !user_present {
            fs::remove_file(home.config().join("preferences.toml")).unwrap();
        }
        let fallback = if user_present {
            "user preferences; no workspace file selected"
        } else {
            "built-in defaults; no workspace file selected"
        };
        for (workspace, expected) in [
            (Some(empty.as_path()), fallback),
            (None, fallback),
            (Some(root.as_path()), "workspace-selected"),
        ] {
            let mut args = vec!["--set", "models.compute=off", "mcp"];
            if let Some(workspace) = workspace {
                args.extend(["--workspace", workspace.to_str().unwrap()]);
            }
            let mut command = home.command(&args);
            command.current_dir(&root);
            let (child, mut input) = super::support::Running::with_stdin(command);
            initialize_mcp(&mut input);
            drop(input);
            let output = child.finish();
            assert_eq!(output.code, Some(0), "{output:?}");
            let response: serde_json::Value =
                serde_json::from_str(output.stdout.lines().next().unwrap()).unwrap();
            let instructions = response["result"]["instructions"].as_str().unwrap();
            assert!(instructions.contains(expected), "{instructions}");
            assert!(
                instructions.contains("require --workspace"),
                "{instructions}"
            );
            assert!(!instructions.contains(home.root().to_str().unwrap()));
            assert!(!instructions.contains(root.to_str().unwrap()));
        }
    }
}

#[test]
fn init_never_parses_an_unsafe_candidate_skipped_by_session() {
    let home = Home::bare();
    let root = project(&home, "malformed planted file");
    make_other_writable(&root.join(".maestro/config.toml"));
    let catalog = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalog");
    let output = home.run_in(
        &root,
        &[
            "--json",
            "--set",
            "language=en",
            "init",
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "knowledge-client",
        ],
    );
    // File-plan preview may refuse replacing unowned bytes, but never as a TOML parse failure.
    assert!(output.stderr.contains("skipped"), "{output:?}");
    assert!(!output.stderr.contains("TOML"), "{output:?}");
    assert!(!output.stderr.contains("invalid"), "{output:?}");
}

#[test]
fn language_and_tone_flags_are_explicit_only_and_cannot_mask_invalid_files() {
    let home = Home::bare();
    let root = project(
        &home,
        "schema = 'maestro-preferences/1'\nlanguage = 'fr'\ntone = 'brief'\n",
    );
    let absent = home.run_in(&root, &["--json", "config", "get", "language"]);
    assert_eq!(absent.code, Some(0), "{absent:?}");
    assert_eq!(absent.json()["value"], "fr");
    let explicit = home.run_in(
        &root,
        &[
            "--json",
            "--language",
            "JA",
            "--tone",
            "detailed",
            "config",
            "get",
            "language",
        ],
    );
    assert_eq!(explicit.code, Some(0), "{explicit:?}");
    assert_eq!(explicit.json()["value"], "ja");
    assert_eq!(explicit.json()["source"], json!({"layer": "--set"}));
    let tone = home.run_in(
        &root,
        &["--json", "--tone", "detailed", "config", "get", "tone"],
    );
    assert_eq!(tone.code, Some(0), "{tone:?}");
    assert_eq!(tone.json()["value"], "detailed");
    for flags in [["--language", "en-US-private"], ["--tone", "chatty"]] {
        let refused = home.run_in(&root, &[&flags[..], &["config", "get", "tone"]].concat());
        assert_eq!(refused.code, Some(2), "{refused:?}");
    }
    fs::write(
        root.join(".maestro/config.toml"),
        "schema = 'maestro-preferences/1'\nunknown = true\n",
    )
    .unwrap();
    let masked = home.run_in(&root, &["--language", "ja", "config", "get", "language"]);
    assert_eq!(masked.code, Some(2), "{masked:?}");
    assert!(masked.stderr.contains("unknown"), "{masked:?}");
}

/// Plant real other-principal write permissions, not a mocked metadata result.
#[cfg(unix)]
fn make_other_writable(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o666)).unwrap();
}

/// Everyone write on the real Windows DACL, checked through held handles at startup.
#[cfg(windows)]
fn make_other_writable(path: &Path) {
    use std::process::Command;
    assert!(
        Command::new("icacls")
            .arg(path)
            .args(["/grant", "*S-1-1-0:(W)"])
            .status()
            .unwrap()
            .success()
    );
}
