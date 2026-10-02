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
