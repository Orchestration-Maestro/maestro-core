//! `maestro init`: inert fixture composition, preview-only default and owned apply.
use super::support::Home;
use std::{fs, path::PathBuf};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalog")
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
}
