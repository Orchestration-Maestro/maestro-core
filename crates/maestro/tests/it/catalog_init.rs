//! `maestro init`: inert fixture composition, preview-only default and owned apply.
use super::support::{Home, Running};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog/bootstrap/owner-local")
        .canonicalize()
        .unwrap()
}

#[test]
fn catalog_init_previews_without_writes_or_script_execution() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir_all(&root).unwrap();
    let marker = root.join("script-ran");
    fs::write(root.join("build.rs"), format!("touch {}", marker.display())).unwrap();
    let catalog = fixtures();
    let result = home.run_in(
        &root,
        &[
            "init",
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "base",
            "--preset",
            "rust",
        ],
    );
    assert_eq!(result.code, Some(0), "{result:?}");
    assert!(
        result
            .stdout
            .contains("authoring convenience; not a verified install")
    );
    let document: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
    assert_eq!(
        document["bindings"],
        serde_json::json!(["tool:common/inspect", "tool:rust/check"])
    );
    assert_eq!(document["applied"], false);
    assert_eq!(document["already_applied"], false);
    assert!(document["files"].get("applied").is_none());
    assert!(!root.join(".maestro/project.toml").exists());
    assert!(!root.join(".github/copilot-instructions.md").exists());
    assert!(!marker.exists());
}

#[test]
fn catalog_init_refuses_changed_sources_after_bootstrap() {
    let home = Home::bare();
    let root = home.root().join("project");
    let catalog = home.root().join("catalog");
    fs::create_dir_all(&root).unwrap();
    copy_tree(&fixtures(), &catalog);
    let guide = root.join(".github/copilot-instructions.md");
    let args = [
        "init",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
        "--apply",
    ];
    approve(&home, &root);
    let initial = home.run_in(&root, &args);
    assert_eq!(initial.code, Some(0), "{initial:?}");
    let before = fs::read(&guide).unwrap();
    for path in [
        "package.toml",
        "core/agents/maestro.maestro.toml",
        "bootstrap/base/files/instructions.md",
    ] {
        let source = catalog.join(path);
        let original = fs::read(&source).unwrap();
        let mut edited = original.clone();
        edited.push(b'\n');
        fs::write(&source, edited).unwrap();
        let changed = home.run_in(&root, &args);
        assert_eq!(changed.code, Some(2), "{path}: {changed:?}");
        assert_eq!(fs::read(&guide).unwrap(), before);
        fs::write(source, original).unwrap();
    }
    assert_eq!(home.run_in(&root, &args).code, Some(0));
}

#[test]
fn catalog_init_apply_writes_composition_and_identical_rerun_is_noop() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir_all(&root).unwrap();
    let catalog = fixtures();
    let args = [
        "init",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
        "--preset",
        "rust",
        "--apply",
    ];
    approve(&home, &root);
    let result = home.run_in(&root, &args);
    assert_eq!(result.code, Some(0), "{result:?}");
    let guide = root.join(".github/copilot-instructions.md");
    let recipes = root.join(".maestro/recipes.json");
    assert!(guide.exists());
    assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(&recipes).unwrap()).is_ok());
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(".maestro/authoring.lock.json")).unwrap())
            .unwrap();
    assert_eq!(lock["sources"].as_array().unwrap().len(), 13);
    assert_eq!(
        lock["areas"],
        serde_json::json!([
            "language:rust",
            "package:common",
            "package:core",
            "standard:quality",
            "standard:security"
        ])
    );
    let guide_bytes = fs::read(&guide).unwrap();
    let rerun = home.run_in(&root, &args);
    assert_eq!(rerun.code, Some(0), "{rerun:?}");
    assert_eq!(fs::read(&guide).unwrap(), guide_bytes);
    assert!(
        rerun.stdout.contains("Already applied; no files written."),
        "{rerun:?}"
    );
}

#[test]
fn catalog_init_reports_found_and_missing_manifest_tools_without_execution() {
    let home = Home::bare();
    let root = home.root().join("project");
    let catalog = home.root().join("catalog");
    fs::create_dir_all(&root).unwrap();
    copy_tree(&fixtures(), &catalog);
    let manifest = catalog.join("bootstrap/base.toml");
    let text = fs::read_to_string(&manifest).unwrap().replace(
        "tools = [\"sh\"]",
        "tools = [\"marker-tool\", \"maestro-c05-missing-tool\"]",
    );
    fs::write(manifest, text).unwrap();
    let bin = home.root().join("bin");
    fs::create_dir(&bin).unwrap();
    let marker = root.join("tool-ran");
    let tool = bin.join(if cfg!(windows) {
        "marker-tool.cmd"
    } else {
        "marker-tool"
    });
    fs::write(&tool, format!("touch {}", marker.display())).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let mut command = home.command(&[
        "--json",
        "init",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
    ]);
    command
        .current_dir(&root)
        .env("PATH", &bin)
        .env("PATHEXT", ".CMD;.EXE");
    let result = Running::of(command).finish();
    assert_eq!(result.code, Some(0), "{result:?}");
    let document: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
    assert_eq!(
        document["prerequisites"],
        serde_json::json!([
            {"tool": "maestro-c05-missing-tool", "found": false},
            {"tool": "marker-tool", "found": true}
        ])
    );
    assert!(!root.join(".github/copilot-instructions.md").exists());
    assert!(!marker.exists());
}

#[cfg(unix)]
#[test]
fn catalog_init_never_claims_applied_when_the_writer_refuses() {
    use std::os::unix::fs::PermissionsExt;
    let home = Home::bare();
    let root = home.root().join("readonly");
    fs::create_dir(&root).unwrap();
    approve(&home, &root);
    fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
    let catalog = fixtures();
    let result = home.run_in(
        &root,
        &[
            "init",
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "base",
            "--apply",
        ],
    );
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stderr.contains("Permission denied"), "{result:?}");
    assert!(result.stdout.contains("\"applied\": false"), "{result:?}");
    assert!(!result.stdout.contains("\"applied\": true"), "{result:?}");
    assert!(!root.join(".maestro/project.toml").exists());
}

#[test]
fn catalog_init_requires_checked_mandatory_and_language_closure() {
    for (path, old, new, message) in [
        (
            "core/agents/maestro.maestro.toml",
            "maturity = \"reviewed\"",
            "maturity = \"authored\"",
            "agent:core/maestro needs reviewed maturity",
        ),
        (
            "presets/rust.toml",
            "requires = [\"language:rust\"]",
            "requires = []",
            "unselected area: rust",
        ),
    ] {
        let home = Home::bare();
        let root = home.root().join("project");
        let catalog = home.root().join("catalog");
        fs::create_dir(&root).unwrap();
        copy_tree(&fixtures(), &catalog);
        let args = [
            "init",
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "rust",
        ];
        let passing = home.run_in(&root, &args);
        assert_eq!(passing.code, Some(0), "{passing:?}");
        let input = catalog.join(path);
        let original = fs::read_to_string(&input).unwrap();
        fs::write(input, original.replace(old, new)).unwrap();
        let refused = home.run_in(&root, &args);
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert!(refused.stderr.contains(message), "{refused:?}");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }
}

#[test]
fn catalog_init_and_check_refuse_unlisted_inventory_payloads() {
    let home = Home::bare();
    let root = home.root().join("project");
    let catalog = home.root().join("catalog");
    fs::create_dir(&root).unwrap();
    copy_tree(&fixtures(), &catalog);
    fs::write(
        catalog.join("bootstrap/base/files/unlisted.md"),
        b"unlisted",
    )
    .unwrap();
    for args in [
        vec![
            "catalog",
            "check",
            "--catalog-dir",
            catalog.to_str().unwrap(),
        ],
        vec![
            "init",
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "base",
        ],
    ] {
        let refused = home.run_in(&root, &args);
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert!(
            refused.stderr.contains("bootstrap/base/files/unlisted.md"),
            "{refused:?}"
        );
        assert!(refused.stderr.contains("not a registered"), "{refused:?}");
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
}

/// Existing apply scenarios explicitly provision user trust before effects.
fn approve(home: &Home, root: &Path) {
    let root = root.canonicalize().unwrap();
    let path = root.to_str().unwrap();
    let result = home.run(&["trust", "add", path, "--confirm-path", path]);
    assert_eq!(result.code, Some(0), "{result:?}");
}

/// Clone only the synthetic checked source tree for editable CLI cases.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let destination = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

#[test]
fn catalog_init_freezes_defaults_for_production_sessions() {
    let home = Home::bare();
    let root = home.root().join("project");
    let catalog = home.root().join("catalog");
    fs::create_dir(&root).unwrap();
    copy_tree(&fixtures(), &catalog);
    fs::create_dir(catalog.join("settings")).unwrap();
    let defaults = catalog.join("settings/defaults.toml");
    fs::write(
        &defaults,
        "schema = 'maestro-preferences/1'\nlanguage = 'fr'\ntone = 'brief'\n",
    )
    .unwrap();
    let graph = catalog.join("core/backends/graphdb/config.toml");
    fs::create_dir_all(graph.parent().unwrap()).unwrap();
    fs::write(
        &graph,
        include_str!("../../../../tests/fixtures/catalog/backends/graphdb.toml")
            .replace("ladybug", "none")
            .replace("max_num_threads = 2", "max_num_threads = 3"),
    )
    .unwrap();
    for (role, text) in [
        (
            "vectordb",
            include_str!("../../../../tests/fixtures/catalog/backends/vectordb.toml"),
        ),
        (
            "mcp",
            include_str!("../../../../tests/fixtures/catalog/backends/mcp.toml"),
        ),
    ] {
        let path = catalog.join(format!("core/backends/{role}/config.toml"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    approve(&home, &root);
    let args = [
        "init",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
        "--apply",
    ];
    let applied = home.run_in(&root, &args);
    assert_eq!(applied.code, Some(0), "{applied:?}");
    let config = fs::read_to_string(root.join(".maestro/config.toml")).unwrap();
    assert!(config.contains("language = \"fr\""), "{config}");
    assert!(config.contains("tone = \"brief\""), "{config}");
    fs::write(
        defaults,
        "schema = 'maestro-preferences/1'\nlanguage = 'ja'\n",
    )
    .unwrap();
    fs::write(graph, "changed catalog bytes").unwrap();
    for (key, value) in [
        ("graph.engine", serde_json::json!("none")),
        ("graphdb.max_num_threads", serde_json::json!(3)),
    ] {
        let result = home.run_in(&root, &["--json", "config", "get", key]);
        assert_eq!(result.code, Some(0), "{result:?}");
        assert_eq!(result.json()["value"], value);
        assert_eq!(
            result.json()["source"],
            serde_json::json!({"layer":"default"})
        );
    }
    let lock = root.join(".maestro/authoring.lock.json");
    let original = fs::read(&lock).unwrap();
    fs::write(&lock, [original.as_slice(), b"\n"].concat()).unwrap();
    let refused = home.run_in(&root, &["--json", "config", "get", "graph.engine"]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(
        refused.stderr.contains(".maestro/authoring.lock.json"),
        "{refused:?}"
    );
    assert!(
        refused.stderr.contains("then run maestro init"),
        "{refused:?}"
    );
    fs::write(lock, original).unwrap();
    let revoked = home.run(&["trust", "remove", root.to_str().unwrap()]);
    assert_eq!(revoked.code, Some(0), "{revoked:?}");
    let refused = home.run_in(&root, &["config", "get", "graph.engine"]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(
        refused.stderr.contains("trusted containing root"),
        "{refused:?}"
    );
}

#[test]
fn catalog_runtime_engine_refuses_uncompiled_flags_without_effects() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let refused = home.run_in(
        &root,
        &[
            "--set",
            "graph.engine=ladybug",
            "config",
            "get",
            "graph.engine",
        ],
    );
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(refused.stderr.contains("not compiled"), "{refused:?}");
    let disabled = home.run_in(
        &root,
        &[
            "--json",
            "--set",
            "graph.engine=none",
            "config",
            "get",
            "graph.engine",
        ],
    );
    assert_eq!(disabled.code, Some(0), "{disabled:?}");
    assert_eq!(disabled.json()["value"], "none");
    assert!(!home.data().join("kernel.sqlite3").exists());
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn catalog_init_reaches_its_plan_with_an_unadmitted_lock() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir_all(root.join(".maestro")).unwrap();
    approve(&home, &root);
    let lock = root.join(".maestro/authoring.lock.json");
    fs::write(&lock, b"invalid synthetic lock").unwrap();
    let session = home.run_in(&root, &["config", "get", "language"]);
    assert_eq!(session.code, Some(2), "{session:?}");
    let catalog = fixtures();
    for apply in [false, true] {
        let mut args = vec![
            "init",
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "base",
        ];
        if apply {
            args.push("--apply");
        }
        let result = home.run_in(&root, &args);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(
            result.stderr.contains("invalid authoring lock"),
            "{result:?}"
        );
        assert!(
            result.stderr.contains(".maestro/authoring.lock.json"),
            "{result:?}"
        );
        assert!(
            result.stderr.contains("then run maestro init"),
            "{result:?}"
        );
        assert_eq!(fs::read(&lock).unwrap(), b"invalid synthetic lock");
        assert!(!root.join(".github/copilot-instructions.md").exists());
    }
}
