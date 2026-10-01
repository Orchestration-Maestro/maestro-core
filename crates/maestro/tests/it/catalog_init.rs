//! `maestro init`: inert fixture composition, preview-only default and owned apply.
use super::support::{Home, Running};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog")
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
            "knowledge-client",
            "--preset",
            "rust-service",
        ],
    );
    assert_eq!(result.code, Some(0), "{result:?}");
    assert!(
        result
            .stdout
            .contains("authoring convenience; not a verified install")
    );
    let document: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
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
    for relative in [
        "bootstrap/knowledge-client.toml",
        "bootstrap/base/.github/copilot-instructions.md",
    ] {
        let destination = catalog.join(relative);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(fixtures().join(relative), destination).unwrap();
    }
    let guide = root.join(".github/copilot-instructions.md");
    let args = [
        "init",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "knowledge-client",
        "--apply",
    ];
    approve(&home, &root);
    let initial = home.run_in(&root, &args);
    assert_eq!(initial.code, Some(0), "{initial:?}");
    let before = fs::read(&guide).unwrap();
    fs::write(
        catalog.join("bootstrap/base/.github/copilot-instructions.md"),
        b"changed source\n",
    )
    .unwrap();
    let changed = home.run_in(&root, &args);
    assert_eq!(changed.code, Some(2), "{changed:?}");
    assert_eq!(fs::read(guide).unwrap(), before);
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
        "knowledge-client",
        "--preset",
        "rust-service",
        "--apply",
    ];
    approve(&home, &root);
    let result = home.run_in(&root, &args);
    assert_eq!(result.code, Some(0), "{result:?}");
    let guide = root.join(".github/copilot-instructions.md");
    let recipes = root.join(".maestro/recipes.json");
    assert!(guide.exists());
    assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(&recipes).unwrap()).is_ok());
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
    fs::create_dir_all(catalog.join("bootstrap/custom")).unwrap();
    fs::write(
        catalog.join("bootstrap/custom.toml"),
        concat!(
            "name = \"custom\"\noverlay = \"custom\"\n",
            "files = [\"custom/target.md\"]\n",
            "tools = [\"marker-tool\", \"maestro-c05-missing-tool\"]\n"
        ),
    )
    .unwrap();
    fs::write(catalog.join("bootstrap/custom/target.md"), b"inert").unwrap();
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
        "custom",
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
    assert!(!root.join("target.md").exists());
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
            "knowledge-client",
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

/// Existing apply scenarios explicitly provision user trust before effects.
fn approve(home: &Home, root: &Path) {
    let root = root.canonicalize().unwrap();
    let path = root.to_str().unwrap();
    let result = home.run(&["trust", "add", path, "--confirm-path", path]);
    assert_eq!(result.code, Some(0), "{result:?}");
}
