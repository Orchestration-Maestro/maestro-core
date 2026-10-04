//! Explicit user trust changes never consult or rewrite preference files.
use super::support::Home;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn root(home: &Home) -> PathBuf {
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    root.canonicalize().unwrap()
}

fn add(home: &Home, root: &Path) -> super::support::Ended {
    let path = root.to_str().unwrap();
    home.run(&["trust", "add", path, "--confirm-path", path])
}

fn init(home: &Home, root: &Path, extra: &[&str]) -> super::support::Ended {
    let catalog = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog/bootstrap/owner-local")
        .canonicalize()
        .unwrap();
    let mut args = vec![
        "init",
        "--yes",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
        "--apply",
    ];
    args.extend(extra);
    home.run_in(root, &args)
}

#[test]
fn review_probe_trusted_default_init_after_preferences_only_config() {
    let home = Home::bare();
    let root = root(&home);
    let path = root.to_str().unwrap();
    let written = init(
        &home,
        &root,
        &["--preferences-only", "--confirm-path", path],
    );
    assert_eq!(written.code, Some(0), "{written:?}");
    let config = fs::read(root.join(".maestro/config.toml")).unwrap();
    assert_eq!(add(&home, &root).code, Some(0));
    let applied = init(&home, &root, &[]);
    assert_eq!(applied.code, Some(0), "{applied:?}");
    assert!(root.join(".maestro/project.toml").exists());
    assert_eq!(fs::read(root.join(".maestro/config.toml")).unwrap(), config);
}

#[test]
fn catalog_workspace_trust_confirmation_is_exact_and_required() {
    let home = Home::bare();
    let root = root(&home);
    let path = root.to_str().unwrap();
    let missing = home.run(&["trust", "add", path]);
    assert_eq!(missing.code, Some(2), "{missing:?}");
    #[cfg(not(windows))]
    assert!(
        missing.stderr.contains(&format!(
            "maestro trust add \"{path}\" --confirm-path \"{path}\""
        )),
        "{missing:?}"
    );
    for confirmation in [home.root().to_str().unwrap(), "project"] {
        let refused = home.run(&["trust", "add", path, "--confirm-path", confirmation]);
        assert_eq!(refused.code, Some(2), "{refused:?}");
    }
    assert_eq!(add(&home, &root).code, Some(0));
}

#[test]
fn catalog_workspace_trust_changes_preserve_invalid_edited_preferences() {
    let home = Home::bare();
    let root = home.root().join("project\\edge");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    fs::create_dir(root.join(".maestro")).unwrap();
    let config = root.join(".maestro/config.toml");
    fs::write(&config, b"edited invalid preferences [trust] yes").unwrap();
    let before = fs::read(&config).unwrap();
    assert_eq!(add(&home, &root).code, Some(0));
    let listed = home.run_in(&root, &["--json", "trust", "list"]);
    assert_eq!(listed.code, Some(0), "{listed:?}");
    let records: serde_json::Value = serde_json::from_str(&listed.stdout).unwrap();
    assert!(
        records
            .as_array()
            .unwrap()
            .iter()
            .any(|record| { record["change"]["path"].as_str() == root.to_str() }),
        "{listed:?}"
    );
    let removed = home.run_in(&root, &["trust", "remove", root.to_str().unwrap()]);
    assert_eq!(removed.code, Some(0), "{removed:?}");
    assert_eq!(fs::read(config).unwrap(), before);
}

#[test]
fn catalog_workspace_trust_roots_refuse_beside_valid_directory() {
    let home = Home::bare();
    let root = root(&home);
    let filesystem_root = root.ancestors().last().unwrap();
    for forbidden in [filesystem_root, home.root(), &home.data(), &home.config()] {
        let refused = add(&home, forbidden);
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert!(refused.stderr.contains("cannot trust"), "{refused:?}");
    }
    assert_eq!(add(&home, &root).code, Some(0));
}

#[test]
fn catalog_workspace_trust_text_json_yes_and_environment_never_approve() {
    let home = Home::bare();
    let root = root(&home);
    let path = root.to_str().unwrap();
    for extra in ["--yes", "--json"] {
        let refused = home.run(&["trust", "add", path, extra]);
        assert_eq!(refused.code, Some(2), "{refused:?}");
    }
    let result = home
        .command(&["trust", "add", path])
        .env("MAESTRO_TRUST", path)
        .env("MAESTRO_CONFIRM_PATH", path)
        .env("MAESTRO_YES", "true")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(home.database().workspace_answers().unwrap().is_empty());
    fs::create_dir(root.join(".maestro")).unwrap();
    fs::write(
        root.join(".maestro/config.toml"),
        "schema = 'maestro-preferences/1'\nlanguage = 'fr'\n",
    )
    .unwrap();
    let listed = home.run(&["--json", "trust", "list"]);
    assert_eq!(listed.code, Some(0), "{listed:?}");
    let records: serde_json::Value = serde_json::from_str(&listed.stdout).unwrap();
    assert!(
        records
            .as_array()
            .unwrap()
            .iter()
            .all(|record| { record["change"]["path"].as_str() != Some(path) }),
        "{listed:?}"
    );
}

#[test]
fn catalog_workspace_trust_revocation_blocks_subsequent_init() {
    let home = Home::bare();
    let root = root(&home);
    let refused = init(&home, &root, &[]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(!root.join(".maestro-files").exists());
    assert_eq!(add(&home, &root).code, Some(0));
    let applied = init(&home, &root, &[]);
    assert_eq!(applied.code, Some(0), "{applied:?}");
    assert!(root.join(".maestro/project.toml").exists());
    let removed = home.run(&["trust", "remove", root.to_str().unwrap()]);
    assert_eq!(removed.code, Some(0), "{removed:?}");
    let revoked = init(&home, &root, &[]);
    assert_eq!(revoked.code, Some(2), "{revoked:?}");
}

#[test]
fn catalog_workspace_trust_preferences_only_requires_separate_confirmation() {
    let home = Home::bare();
    let root = root(&home);
    for extra in [
        vec!["--preferences-only"],
        vec![
            "--preferences-only",
            "--confirm-path",
            home.root().to_str().unwrap(),
        ],
        vec!["--preferences-only", "--yes"],
    ] {
        let refused = init(&home, &root, &extra);
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }
    let applied = init(
        &home,
        &root,
        &[
            "--preferences-only",
            "--confirm-path",
            root.to_str().unwrap(),
        ],
    );
    assert_eq!(applied.code, Some(0), "{applied:?}");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    assert_eq!(fs::read_dir(root.join(".maestro")).unwrap().count(), 1);
    assert!(root.join(".maestro/config.toml").exists());
    let listed = home.run(&["--json", "trust", "list"]);
    assert_eq!(listed.code, Some(0), "{listed:?}");
    let records: serde_json::Value = serde_json::from_str(&listed.stdout).unwrap();
    assert!(
        records
            .as_array()
            .unwrap()
            .iter()
            .all(|record| { record["change"]["path"].as_str() != root.to_str() }),
        "{listed:?}"
    );
    assert_eq!(init(&home, &root, &[]).code, Some(2));
}

#[test]
fn catalog_workspace_trust_approved_init_writes_selected_preferences() {
    let home = Home::bare();
    let root = root(&home);
    assert_eq!(add(&home, &root).code, Some(0));
    let applied = init(&home, &root, &["--language", "fr", "--tone", "brief"]);
    assert_eq!(applied.code, Some(0), "{applied:?}");
    let config = root.join(".maestro/config.toml");
    let before = fs::read(&config).unwrap();
    let text = String::from_utf8(before.clone()).unwrap();
    assert!(text.contains("language = \"fr\""));
    assert!(text.contains("tone = \"brief\""));
    let receipt = home.database().trusted_workspaces().unwrap();
    assert_eq!(receipt.len(), 1);
    assert!(!receipt[0].id.is_empty());
    let repeat = init(&home, &root, &["--language", "fr", "--tone", "brief"]);
    assert_eq!(repeat.code, Some(0), "{repeat:?}");
    assert_eq!(fs::read(&config).unwrap(), before);
}

#[test]
fn catalog_workspace_trust_external_discovery_uses_journal_not_copied_config() {
    let home = Home::bare();
    let outside = Home::bare();
    let root = root(&outside);
    fs::create_dir(root.join(".maestro")).unwrap();
    fs::write(
        root.join(".maestro/config.toml"),
        "schema = 'maestro-preferences/1'\nlanguage = 'fr'\n",
    )
    .unwrap();
    super::support::make_safe_preferences_path(&root.join(".maestro"));
    super::support::make_safe_preferences_path(&root.join(".maestro/config.toml"));
    let language = || home.run_in(&root, &["config", "get", "language"]);
    assert_ne!(language().stdout.trim(), "fr");
    assert_eq!(add(&home, &root).code, Some(0));
    let selected = language();
    assert_eq!(selected.code, Some(0), "{selected:?}");
    assert_eq!(selected.stdout.trim(), "fr");
    assert_eq!(
        home.run(&["trust", "remove", root.to_str().unwrap()]).code,
        Some(0)
    );
    assert_ne!(language().stdout.trim(), "fr");
}

#[test]
fn catalog_workspace_trust_mcp_text_has_no_approval_surface() {
    use std::io::Write as _;
    let home = Home::bare();
    let root = root(&home);
    let (child, mut input) = home.start_with_stdin(&["mcp"]);
    super::support::initialize_mcp(&mut input);
    writeln!(
        input,
        "{}",
        serde_json::json!({"jsonrpc":"2.0", "method":"notifications/initialized"})
    )
    .unwrap();
    writeln!(
        input,
        "{}",
        serde_json::json!({"jsonrpc":"2.0", "id":2, "method":"tools/list", "params":{}})
    )
    .unwrap();
    writeln!(
        input,
        "{}",
        serde_json::json!({
            "jsonrpc":"2.0", "id":3, "method":"tools/call",
            "params":{"name":"trust_add", "arguments":{
                "path":root, "confirm_path":root, "approved":true
            }}
        })
    )
    .unwrap();
    drop(input);
    let result = child.finish();
    assert_eq!(result.code, Some(0), "{result:?}");
    let responses: Vec<serde_json::Value> = result
        .stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let tools = responses
        .iter()
        .find(|response| response["id"] == 2)
        .unwrap()["result"]["tools"]
        .as_array()
        .unwrap();
    assert!(
        tools
            .iter()
            .all(|tool| !tool["name"].as_str().unwrap().contains("trust"))
    );
    assert!(
        responses
            .iter()
            .find(|response| response["id"] == 3)
            .unwrap()["error"]
            .is_object()
    );
    assert!(home.database().workspace_answers().unwrap().is_empty());
}

#[test]
fn catalog_workspace_trust_unreadable_authority_is_untrusted_without_writes() {
    let home = Home::bare();
    let outside = Home::bare();
    let root = root(&outside);
    fs::create_dir(root.join(".maestro")).unwrap();
    let config = root.join(".maestro/config.toml");
    fs::write(
        &config,
        "schema = 'maestro-preferences/1'\nlanguage = 'fr'\n",
    )
    .unwrap();
    super::support::make_safe_preferences_path(&root.join(".maestro"));
    super::support::make_safe_preferences_path(&config);
    let path = home.data().join("kernel.sqlite3");
    fs::write(&path, b"unreadable authority").unwrap();
    let before = fs::read(&config).unwrap();
    let selected = home.run_in(&root, &["config", "get", "language"]);
    assert_eq!(selected.code, Some(0), "{selected:?}");
    assert_ne!(selected.stdout.trim(), "fr");
    assert_eq!(fs::read(&path).unwrap(), b"unreadable authority");
    assert_eq!(fs::read_dir(home.data()).unwrap().count(), 1);
    assert_eq!(fs::read(&config).unwrap(), before);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}

#[test]
fn catalog_workspace_trust_list_does_not_create_a_fresh_kernel() {
    let home = Home::bare();
    let listed = home.run(&["--json", "trust", "list"]);
    assert_eq!(listed.code, Some(0), "{listed:?}");
    assert_eq!(listed.stdout.trim(), "[]");
    assert_eq!(fs::read_dir(home.data()).unwrap().count(), 0);
}

#[test]
fn catalog_workspace_trust_session_does_not_create_missing_authority() {
    let home = Home::bare();
    let selected = home.run(&["config", "get", "language"]);
    assert_eq!(selected.code, Some(0), "{selected:?}");
    assert_eq!(fs::read_dir(home.data()).unwrap().count(), 0);
}

#[test]
fn catalog_workspace_trust_suggestions_quote_spaces_and_dot_resolves_selected_folder() {
    let home = Home::bare();
    let root = home.root().join("équipe project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let path = root.to_str().unwrap();
    let missing = home.run_in(&root, &["trust", "add", "."]);
    assert_eq!(missing.code, Some(2), "{missing:?}");
    assert!(
        missing
            .stderr
            .contains("équipe project\" --confirm-path \""),
        "{missing:?}"
    );
    let approved = home.run_in(&root, &["trust", "add", ".", "--confirm-path", path]);
    assert_eq!(approved.code, Some(0), "{approved:?}");
    assert_eq!(
        home.database().trusted_workspaces().unwrap()[0].change.path,
        root
    );
}

#[cfg(windows)]
#[test]
fn catalog_workspace_trust_accepts_lossless_plain_windows_confirmation_only() {
    let home = Home::bare();
    let root = root(&home);
    let raw = root.to_str().unwrap();
    let plain = raw.strip_prefix(r"\\?\").unwrap();
    let approved = home.run(&["trust", "add", raw, "--confirm-path", plain]);
    assert_eq!(approved.code, Some(0), "{approved:?}");
    for spelling in [".", "project"] {
        let refused = home.run(&["trust", "add", raw, "--confirm-path", spelling]);
        assert_eq!(refused.code, Some(2), "{refused:?}");
    }
    assert_eq!(
        home.database().trusted_workspaces().unwrap()[0].change.path,
        root
    );
}

// util-linux script gives the child a real terminal; other platforms test confirmation IO.
#[cfg(target_os = "linux")]
#[test]
fn trust_add_never_uses_terminal_input_when_stderr_is_redirected() {
    use super::support::Running;
    use std::{
        io::Write as _,
        process::{Command, Stdio},
    };
    let home = Home::bare();
    let project = home.root().join("project");
    fs::create_dir(&project).unwrap();
    let errors = home.root().join("trust-errors");
    let invocation = format!(
        "\"{}\" trust add \"{}\" 2>\"{}\"",
        env!("CARGO_BIN_EXE_maestro"),
        project.display(),
        errors.display()
    );
    let mut command = Command::new("script");
    command
        .args(["-q", "-e", "-c", &invocation, "/dev/null"])
        .env("HOME", home.root())
        .env("XDG_DATA_HOME", home.root().join("data"))
        .env("XDG_CONFIG_HOME", home.root().join("config"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (running, mut input) = Running::with_stdin(command);
    input.write_all(b"yes\n").unwrap();
    drop(input);
    let result = running.finish();
    assert_eq!(result.code, Some(2), "{result:?}");
    let stderr = fs::read_to_string(errors).unwrap();
    assert!(!stderr.contains("[y/N]"), "{stderr}");
    assert!(stderr.contains("--confirm-path"), "{stderr}");
}
