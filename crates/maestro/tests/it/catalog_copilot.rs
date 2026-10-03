//! Isolated authoring projections prove delivery/ownership, never host obedience.
use super::support::{Ended, Home, Running};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const PROJECTED: [&str; 4] = [
    ".github/agents/maestro.agent.md",
    ".github/copilot-instructions.md",
    ".maestro/copilot-ownership.json",
    ".mcp.json",
];

fn catalog() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog/bootstrap/owner-local")
        .canonicalize()
        .unwrap()
}

fn project(home: &Home, root: &Path, extra: &[&str], locale: (&str, &str)) -> Ended {
    Running::of(project_command(home, root, extra, locale)).finish()
}

fn project_command(home: &Home, root: &Path, extra: &[&str], locale: (&str, &str)) -> Command {
    let catalog = catalog();
    let mut args = vec![
        "--json",
        "--language",
        locale.0,
        "--tone",
        locale.1,
        "catalog",
        "project",
        "--host",
        "copilot",
        "--target",
        root.to_str().unwrap(),
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
    ];
    args.extend(extra);
    let mut command = home.command(&args);
    command.env("COPILOT_HOME", home.root().join("copilot"));
    command
}

fn trust(home: &Home, root: &Path) {
    let path = root.to_str().unwrap();
    let approved = home.run(&["trust", "add", path, "--confirm-path", path]);
    assert_eq!(approved.code, Some(0), "{approved:?}");
}

#[test]
fn catalog_copilot_dry_run_exact_external_trust_owned_entries_and_removal() {
    let home = Home::bare();
    let outside = Home::bare();
    let root = outside.root().join("project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let preview = project(&home, &root, &[], ("en", "brief"));
    assert_eq!(preview.code, Some(0), "{preview:?}");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert!(!home.data().join("kernel.sqlite3").exists());
    let refused = project(&home, &root, &["--apply"], ("en", "brief"));
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(refused.stderr.contains("maestro trust add"), "{refused:?}");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    trust(&home, &outside.root().canonicalize().unwrap());
    let inherited = project(&home, &root, &["--apply"], ("en", "brief"));
    assert_eq!(inherited.code, Some(2), "{inherited:?}");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    trust(&home, &root);
    owned_roundtrip(&home, &root);
    assert!(!home.root().join("copilot").exists());
}

fn owned_roundtrip(home: &Home, root: &Path) {
    fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers":{"user":{"command":"user"}},"other":true}"#,
    )
    .unwrap();
    let applied = project(home, root, &["--apply"], ("en", "brief"));
    assert_eq!(applied.code, Some(0), "{applied:?}");
    let document: Value = serde_json::from_str(&applied.stdout).unwrap();
    assert_eq!(document["registered"], true);
    assert_eq!(document["observed"], false);
    assert_eq!(document["stale"], serde_json::json!([]));
    assert_eq!(document["failed"], serde_json::json!([]));
    assert!(
        document["diagnosis"]
            .as_str()
            .unwrap()
            .contains("reload inside a running session: not run")
    );
    let initial: Vec<_> = PROJECTED
        .iter()
        .map(|path| fs::read(root.join(path)).unwrap())
        .collect();
    let replay = project(home, root, &["--apply"], ("ja", "detailed"));
    assert_eq!(replay.code, Some(0), "{replay:?}");
    assert_eq!(
        initial,
        PROJECTED
            .iter()
            .map(|path| fs::read(root.join(path)).unwrap())
            .collect::<Vec<_>>()
    );
    owned_remove(home, root);
}

fn owned_remove(home: &Home, root: &Path) {
    let removed = project(home, root, &["--remove", "--apply"], ("en", "brief"));
    assert_eq!(removed.code, Some(0), "{removed:?}");
    for path in &PROJECTED[..3] {
        assert!(!root.join(path).exists());
    }
    let shared: Value = serde_json::from_slice(&fs::read(root.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(shared["mcpServers"]["user"]["command"], "user");
    assert_eq!(shared["other"], true);
    assert!(shared["mcpServers"].get("maestro").is_none());
    let again = project(home, root, &["--remove", "--apply"], ("en", "brief"));
    assert_eq!(again.code, Some(0), "{again:?}");
    assert!(!home.root().join("copilot").exists());
}

#[test]
fn catalog_copilot_host_owned_home_set_unset_and_missing_are_read_only() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let missing = project(&home, &root, &[], ("en", "brief"));
    assert_eq!(missing.code, Some(0), "{missing:?}");
    assert!(!home.root().join("copilot").exists());
    let profile = fs::read(catalog().join("core/agents/maestro.agent.md")).unwrap();
    let overridden = home.root().join("copilot/agents");
    fs::create_dir_all(&overridden).unwrap();
    fs::write(overridden.join("renamed.agent.md"), &profile).unwrap();
    let set = project(&home, &root, &[], ("en", "brief"));
    assert_eq!(set.code, Some(2), "{set:?}");
    assert!(set.stderr.contains("renamed.agent.md"), "{set:?}");
    let default = home.root().join(".copilot/agents");
    fs::create_dir_all(&default).unwrap();
    fs::write(default.join("default.agent.md"), &profile).unwrap();
    let mut command = project_command(&home, &root, &[], ("en", "brief"));
    command.env_remove("COPILOT_HOME");
    let unset = Running::of(command).finish();
    assert_eq!(unset.code, Some(2), "{unset:?}");
    assert!(unset.stderr.contains("default.agent.md"), "{unset:?}");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert_eq!(
        fs::read(overridden.join("renamed.agent.md")).unwrap(),
        profile
    );
    assert_eq!(fs::read(default.join("default.agent.md")).unwrap(), profile);
}

#[test]
fn catalog_copilot_all_languages_and_tones_write_identical_native_files() {
    let mut baseline = None;
    for language in ["en", "fr", "es", "ja"] {
        for tone in ["brief", "normal", "detailed"] {
            let home = Home::bare();
            let root = home.root().join("project");
            fs::create_dir(&root).unwrap();
            let root = root.canonicalize().unwrap();
            trust(&home, &root);
            let result = project(&home, &root, &["--apply"], (language, tone));
            assert_eq!(result.code, Some(0), "{language}/{tone}: {result:?}");
            let bytes: Vec<_> = PROJECTED
                .iter()
                .map(|path| fs::read(root.join(path)).unwrap())
                .collect();
            if let Some(expected) = &baseline {
                assert_eq!(&bytes, expected, "{language}/{tone}");
            } else {
                baseline = Some(bytes.clone());
            }
            let instructions = String::from_utf8(bytes[1].clone()).unwrap();
            assert!(
                instructions
                    .contains("Follow the current MCP session's conversation language and tone.")
            );
            assert!(instructions.contains(
                "Keep code, commits, names, identifiers, logs and documentation in English."
            ));
            for value in [
                "Conversation language:",
                "tone:",
                "brief",
                "normal",
                "detailed",
            ] {
                assert!(!instructions.contains(value));
            }
        }
    }
}

#[test]
fn catalog_copilot_missing_sources_unknown_presets_and_ambiguous_host_refuse() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let valid = project(&home, &root, &[], ("en", "normal"));
    assert_eq!(valid.code, Some(0), "{valid:?}");
    let checked_catalog = catalog();
    for args in [
        vec!["catalog", "project", "--host", "copilot"],
        vec![
            "catalog",
            "project",
            "--host",
            "copilot,pi",
            "--target",
            root.to_str().unwrap(),
            "--catalog-dir",
            checked_catalog.to_str().unwrap(),
            "--preset",
            "base",
        ],
    ] {
        let result = home.run(&args);
        assert_ne!(result.code, Some(0), "{result:?}");
    }
    let unknown = project(&home, &root, &["--preset", "unknown"], ("en", "normal"));
    assert_ne!(unknown.code, Some(0), "{unknown:?}");
    let missing = home.run(&[
        "catalog",
        "project",
        "--host",
        "copilot",
        "--target",
        root.to_str().unwrap(),
        "--catalog-dir",
        "missing",
        "--preset",
        "base",
    ]);
    assert_ne!(missing.code, Some(0), "{missing:?}");
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn catalog_copilot_reports_replacement_race_limit() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    trust(&home, &root);
    for extra in [&[][..], &["--apply"][..]] {
        let result = project(&home, &root, extra, ("en", "brief"));
        assert_eq!(result.code, Some(0), "{result:?}");
        let document: Value = serde_json::from_str(&result.stdout).unwrap();
        let diagnosis = document["diagnosis"].as_str().unwrap();
        assert!(diagnosis.contains("replacement is not compare-and-swap"));
        assert!(diagnosis.contains("avoid concurrent edits of shared configuration"));
    }
}

#[test]
fn catalog_copilot_failure_states() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    trust(&home, &root);
    // Malformed user data is failed, not stale.
    fs::write(root.join(".mcp.json"), b"invalid").unwrap();
    let result = project(&home, &root, &[], ("en", "brief"));
    assert_eq!(result.code, Some(2), "{result:?}");
    let document: Value = serde_json::from_str(&result.stdout).unwrap();
    assert_eq!(document["registered"], false);
    assert_eq!(document["observed"], false);
    assert_eq!(document["stale"].as_array().unwrap().len(), 0);
    assert_eq!(document["failed"].as_array().unwrap().len(), 1);
    fs::remove_file(root.join(".mcp.json")).unwrap();
    assert_eq!(
        project(&home, &root, &["--apply"], ("en", "brief")).code,
        Some(0)
    );
    // Entry ownership drift and owned native-file drift independently classify as stale.
    let shared = fs::read(root.join(".mcp.json")).unwrap();
    fs::write(root.join(".mcp.json"), b"{\"mcpServers\":{}}").unwrap();
    let entry = project(&home, &root, &[], ("en", "brief"));
    assert_stale(&entry);
    fs::write(root.join(".mcp.json"), shared).unwrap();
    fs::write(root.join(".github/agents/maestro.agent.md"), b"changed").unwrap();
    assert_stale(&project(&home, &root, &[], ("en", "brief")));
}

fn assert_stale(result: &Ended) {
    assert_eq!(result.code, Some(2), "{result:?}");
    let document: Value = serde_json::from_str(&result.stdout).unwrap();
    assert_eq!(document["registered"], false);
    assert_eq!(document["observed"], false);
    assert_eq!(document["stale"].as_array().unwrap().len(), 1);
    assert_eq!(document["failed"].as_array().unwrap().len(), 0);
}

#[test]
fn catalog_copilot_text_trust_refusal_keeps_target_empty() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let source = catalog();
    let mut command = home.command(&[
        "catalog",
        "project",
        "--host",
        "copilot",
        "--target",
        root.to_str().unwrap(),
        "--catalog-dir",
        source.to_str().unwrap(),
        "--preset",
        "base",
        "--apply",
    ]);
    command.env("COPILOT_HOME", home.root().join("copilot"));
    let result = Running::of(command).finish();
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stderr.contains("maestro trust add"), "{result:?}");
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn catalog_copilot_output_failure_is_not_an_effect_receipt() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    fs::write(root.join(".mcp.json"), b"invalid").unwrap();
    let mut command = project_command(&home, &root, &[], ("en", "brief"));
    let mut child = command.spawn().unwrap();
    drop(child.stdout.take());
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read(root.join(".mcp.json")).unwrap(), b"invalid");
    assert!(!root.join(".github").exists());
}
