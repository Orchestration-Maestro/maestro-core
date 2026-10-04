//! Empty optional ports, captured absence and version-specific recovery.
use super::{
    inventory::Fixture,
    support::{apply, preview},
};
use crate::bootstrap::{Preset, PresetPort};
use std::{collections::BTreeSet, fs};

/// An inert external port using both optional defaults.
struct EmptyPort;
impl PresetPort for EmptyPort {
    fn resolve(&self, _: &[String]) -> Result<Vec<Preset>, String> {
        Ok(Vec::new())
    }
}

#[test]
fn preset_optional_defaults_are_empty() {
    assert!(EmptyPort.absent_inputs().is_empty());
    assert_eq!(EmptyPort.backend_types(), BTreeSet::new());
}

#[test]
fn inventory_absence_pins_defaults_separately_from_backend_configs() {
    let fixture = Fixture::new();
    let port = fixture.port().unwrap();
    let absent = port.absent_inputs();
    assert!(
        absent
            .iter()
            .any(|(_, path)| path == "settings/defaults.toml"),
        "{absent:?}"
    );
    assert!(
        absent
            .iter()
            .any(|(_, path)| path.starts_with("core/backends/")),
        "{absent:?}"
    );
    fs::create_dir(fixture.catalog.join("settings")).unwrap();
    fs::write(
        fixture.catalog.join("settings/defaults.toml"),
        "schema = 'maestro-preferences/1'\nlanguage = 'fr'\n",
    )
    .unwrap();
    let with_defaults = fixture.port().unwrap().absent_inputs();
    assert!(
        !with_defaults
            .iter()
            .any(|(_, path)| path == "settings/defaults.toml")
    );
    assert!(
        with_defaults
            .iter()
            .any(|(_, path)| path.starts_with("core/backends/")),
        "{with_defaults:?}"
    );
    let proposal = preview(&fixture.project, &port, &["base".into()]).unwrap();
    fs::remove_file(fixture.catalog.join("settings/defaults.toml")).unwrap();
    let (_, missing) = absent
        .iter()
        .find(|(_, path)| path.starts_with("core/backends/"))
        .unwrap();
    let parent = fixture
        .catalog
        .join(missing)
        .parent()
        .unwrap()
        .to_path_buf();
    fs::create_dir_all(parent.parent().unwrap()).unwrap();
    fs::write(&parent, b"not a directory").unwrap();
    let error = apply(&fixture.project, &proposal).unwrap_err();
    assert!(error.to_string().contains("input changed"), "{error}");
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
}

#[test]
fn old_version_two_lock_names_its_required_fresh_preview() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.project.join(".maestro")).unwrap();
    fs::write(
        fixture.project.join(".maestro/authoring.lock.json"),
        br#"{"schema":"maestro-authoring-lock/2"}"#,
    )
    .unwrap();
    let error = preview(&fixture.project, &fixture.port().unwrap(), &["base".into()]).unwrap_err();
    assert!(error.contains("maestro-authoring-lock/2"), "{error}");
    assert!(error.contains("fresh preview"), "{error}");
}

#[test]
fn session_admits_lock_at_exact_aggregate_limit_when_all_outputs_are_missing() {
    use super::session_lock::{admit, initialized};
    use crate::{limits::Limits, settings::WorkspacePreferences as _};
    let fixture = initialized();
    let bytes = fs::read(fixture.project.join(".maestro/authoring.lock.json")).unwrap();
    let lock: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for file in lock["files"].as_array().unwrap() {
        fs::remove_file(fixture.project.join(file["path"].as_str().unwrap())).unwrap();
    }
    let limits = Limits {
        archive_total_bytes: u64::try_from(bytes.len()).unwrap(),
        ..Limits::PRODUCTION
    };
    let snapshot = admit(&fixture, &limits).unwrap();
    assert_eq!(
        snapshot
            .registry()
            .unwrap()
            .default_of("language")
            .unwrap()
            .to_string(),
        "fr"
    );
    assert!(
        snapshot
            .discovery
            .note
            .unwrap()
            .contains("generated outputs drifted")
    );
}
