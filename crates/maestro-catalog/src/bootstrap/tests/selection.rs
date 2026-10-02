//! C36: composition admits only the source-checked mandatory closure.
use super::{
    super::PresetPort,
    inventory::Fixture,
    support::{apply, preview},
};
use crate::{
    limits::Limits,
    source::{Directory, Known, builtin, check, frozen_rows},
};
use std::fs;

#[test]
fn mandatory_roots_selected_once() {
    let fixture = Fixture::new();
    let registry = builtin().unwrap();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let rows = frozen_rows();
    let checked = check(
        &Directory::new(&fixture.catalog),
        &registry,
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
    )
    .unwrap();
    let id = checked
        .resources
        .iter()
        .find(|resource| resource.id.kind == "preset" && resource.id.name == "rust")
        .unwrap()
        .id
        .clone();
    let closure = checked.selection(&[id], &registry).unwrap();
    for required in [
        "package:common",
        "package:core",
        "standard:quality",
        "standard:security",
        "language:rust",
        "agent:core/maestro",
    ] {
        assert_eq!(
            closure
                .iter()
                .filter(|resource| resource.id.to_string() == required)
                .count(),
            1,
            "{required}"
        );
    }
    let port = fixture.port().unwrap();
    let proposal = preview(&fixture.project, &port, &["base".into(), "rust".into()]).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    assert_eq!(
        fs::read(fixture.project.join(".github/copilot-instructions.md")).unwrap(),
        b"Synthetic knowledge instructions.\n"
    );
    assert_eq!(
        fs::read(fixture.project.join(".maestro/recipes.json")).unwrap(),
        b"{\"recipes\":[]}\n"
    );
    fixture.edit(
        "core/agents/maestro.maestro.toml",
        "maturity = \"reviewed\"",
        "maturity = \"authored\"",
    );
    let error = fixture
        .port()
        .unwrap()
        .resolve(&["base".into()])
        .unwrap_err();
    assert!(
        error.contains("agent:core/maestro needs reviewed maturity"),
        "{error}"
    );
    fixture.edit(
        "core/agents/maestro.maestro.toml",
        "maturity = \"authored\"",
        "maturity = \"reviewed\"",
    );
    fs::remove_file(fixture.catalog.join("core/package.toml")).unwrap();
    let error = fixture.port().unwrap_err();
    assert!(
        error.contains("core/package.toml") || error.contains("package:core"),
        "{error}"
    );
}

#[test]
fn unselected_inventory_refuses() {
    let fixture = Fixture::new();
    assert!(fixture.port().unwrap().resolve(&["rust".into()]).is_ok());
    fixture.edit(
        "presets/rust.toml",
        "requires = [\"language:rust\"]",
        "requires = []",
    );
    let error = fixture
        .port()
        .unwrap()
        .resolve(&["rust".into()])
        .unwrap_err();
    assert!(error.contains("unselected area: rust"), "{error}");
    assert!(fixture.port().unwrap().resolve(&["base".into()]).is_ok());
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
}

#[test]
fn distinct_inventory_output_collision_refuses() {
    let fixture = Fixture::new();
    let path = fixture.catalog.join("bootstrap/base.toml");
    let inventory = fs::read_to_string(&path).unwrap();
    let distinct = inventory.replace("name = \"base\"", "name = \"second\"");
    fs::write(fixture.catalog.join("bootstrap/second.toml"), distinct).unwrap();
    let files = fixture.catalog.join("bootstrap/second/files");
    fs::create_dir_all(&files).unwrap();
    fs::copy(
        fixture.catalog.join("bootstrap/base/files/instructions.md"),
        files.join("instructions.md"),
    )
    .unwrap();
    fixture.edit(
        "presets/base.toml",
        "common/base",
        "common/base\", \"common/second",
    );
    let error = preview(&fixture.project, &fixture.port().unwrap(), &["base".into()]).unwrap_err();
    assert!(
        error.contains("file collision: .github/copilot-instructions.md"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    fixture.edit(
        "bootstrap/second.toml",
        ".github/copilot-instructions.md",
        "second.md",
    );
    assert!(preview(&fixture.project, &fixture.port().unwrap(), &["base".into()]).is_ok());
}

#[test]
fn transitive_preset_is_composed_and_revalidated() {
    let fixture = Fixture::new();
    let base = fixture.catalog.join("presets/base.toml");
    let text = fs::read_to_string(&base).unwrap();
    fs::write(base, format!("{text}\nrequires = [\"preset:rust\"]\n")).unwrap();
    let port = fixture.port().unwrap();
    let resolved = port.resolve(&["base".into()]).unwrap();
    assert_eq!(
        resolved
            .iter()
            .map(|preset| preset.name.as_str())
            .collect::<Vec<_>>(),
        ["base", "rust"]
    );
    let proposal = preview(&fixture.project, &port, &["base".into()]).unwrap();
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    let rust = fixture.catalog.join("presets/rust.toml");
    let original = fs::read(&rust).unwrap();
    let mut changed = original.clone();
    changed.push(b'\n');
    fs::write(&rust, changed).unwrap();
    let error = apply(&fixture.project, &proposal).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("input changed; run preview again"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    fs::write(rust, original).unwrap();
    let fresh = preview(&fixture.project, &fixture.port().unwrap(), &["base".into()]).unwrap();
    apply(&fixture.project, &fresh).unwrap();
    assert_eq!(
        fs::read(fixture.project.join(".github/copilot-instructions.md")).unwrap(),
        b"Synthetic knowledge instructions.\n"
    );
    assert_eq!(
        fs::read(fixture.project.join(".maestro/recipes.json")).unwrap(),
        b"{\"recipes\":[]}\n"
    );
}
