//! Trusted session defaults reuse genuine C04 ownership, never lock self-authority.
use super::{
    inventory::Fixture,
    support::{apply, preview},
};
use crate::{
    files::tests::support::with_trust,
    limits::Limits,
    policy::workspace::{CheckedTrust, TrustBoundaries},
    settings::{AdmissionError, NoWorkspaceTrust, SessionPreferences, WorkspacePreferences},
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

/// One genuine init lock, independent of live engines and machine settings.
pub(super) fn initialized() -> Fixture {
    let fixture = Fixture::new();
    fs::create_dir(fixture.catalog.join("settings")).unwrap();
    fs::write(
        fixture.catalog.join("settings/defaults.toml"),
        "schema = 'maestro-preferences/1'\nlanguage = 'fr'\n",
    )
    .unwrap();
    let proposal = preview(&fixture.project, &fixture.port().unwrap(), &["base".into()]).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    fixture
}

/// Capture through safe discovery without granting defaults admission.
fn snapshot(fixture: &Fixture) -> SessionPreferences {
    SessionPreferences::load(
        &fixture.root.join("user"),
        Some(&fixture.project),
        Some(&fixture.root),
        &NoWorkspaceTrust,
        &Limits::PRODUCTION,
    )
    .unwrap()
}

/// Production admission with fixture authority, returning the diagnostic for exact tests.
pub(super) fn admit(fixture: &Fixture, limits: &Limits) -> Result<SessionPreferences, String> {
    with_trust(&fixture.project, |trust| {
        snapshot(fixture)
            .admit_defaults(trust, &BTreeSet::new(), limits)
            .map_err(|error| error.to_string())
    })
}

#[test]
fn session_lock_retains_the_complete_identity_without_writing() {
    use crate::files::digest;
    let fixture = initialized();
    let path = fixture.project.join(".maestro/authoring.lock.json");
    let bytes = fs::read(&path).unwrap();
    let before = project_bytes(&fixture.project);
    let admitted = admit(&fixture, &Limits::PRODUCTION).unwrap();
    assert_eq!(admitted.frozen_lock(), Some(digest(&bytes).as_str()));
    assert_eq!(project_bytes(&fixture.project), before);
    fs::write(&path, [bytes.as_slice(), b"\n"].concat()).unwrap();
    let changed = project_bytes(&fixture.project);
    assert!(admit(&fixture, &Limits::PRODUCTION).is_err());
    assert_eq!(project_bytes(&fixture.project), changed);
    assert_eq!(admitted.frozen_lock(), Some(digest(&bytes).as_str()));
    fs::remove_file(&path).unwrap();
    let missing = project_bytes(&fixture.project);
    assert!(admit(&fixture, &Limits::PRODUCTION).is_err());
    assert_eq!(project_bytes(&fixture.project), missing);
}

/// Exact relative names and bytes, including ownership journals, without following links.
fn project_bytes(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn collect(root: &Path, path: &Path, rows: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                collect(root, &entry.path(), rows);
            } else {
                rows.push((
                    entry.path().strip_prefix(root).unwrap().to_owned(),
                    fs::read(entry.path()).unwrap(),
                ));
            }
        }
    }
    let mut rows = Vec::new();
    collect(root, root, &mut rows);
    rows.sort();
    rows
}

#[test]
fn session_lock_requires_trust_beside_legacy_defaults() {
    let fixture = initialized();
    let boundaries = TrustBoundaries::new(&fixture.root, &[]).unwrap();
    let denied = CheckedTrust::new(&NoWorkspaceTrust, &boundaries);
    assert!(
        snapshot(&fixture)
            .admit_defaults(&denied, &BTreeSet::new(), &Limits::PRODUCTION)
            .unwrap_err()
            .to_string()
            .contains("trusted containing root")
    );
    assert_eq!(
        admit(&fixture, &Limits::PRODUCTION)
            .unwrap()
            .registry()
            .unwrap()
            .default_of("language")
            .unwrap()
            .to_string(),
        "fr"
    );
}

#[test]
fn session_changed_or_missing_lock_refuses() {
    let fixture = initialized();
    let path = fixture.project.join(".maestro/authoring.lock.json");
    let original = fs::read(&path).unwrap();
    for bytes in [b"{}".to_vec(), [original.as_slice(), b"\n"].concat()] {
        fs::write(&path, bytes).unwrap();
        assert!(admit(&fixture, &Limits::PRODUCTION).is_err());
    }
    fs::write(&path, &original).unwrap();
    assert!(admit(&fixture, &Limits::PRODUCTION).is_ok());
    fs::remove_file(&path).unwrap();
    assert!(
        admit(&fixture, &Limits::PRODUCTION)
            .unwrap_err()
            .contains("project lock")
    );
}

#[test]
fn session_lock_schema_and_output_bounds_refuse() {
    let fixture = initialized();
    let path = fixture.project.join(".maestro/authoring.lock.json");
    let original = fs::read(&path).unwrap();
    let mut lock: serde_json::Value = serde_json::from_slice(&original).unwrap();
    lock["schema"] = serde_json::json!("maestro-authoring-lock/2");
    lock.as_object_mut().unwrap().remove("defaults");
    fs::write(&path, serde_json::to_vec(&lock).unwrap()).unwrap();
    assert!(
        admit(&fixture, &Limits::PRODUCTION)
            .unwrap_err()
            .contains("unsupported project lock")
    );
    fs::write(&path, &original).unwrap();
    let count = lock["files"].as_array().unwrap().len();
    assert!(
        admit(
            &fixture,
            &Limits {
                archive_entries: count,
                ..Limits::PRODUCTION
            }
        )
        .unwrap_err()
        .contains("output count")
    );
    assert!(
        admit(
            &fixture,
            &Limits {
                archive_total_bytes: 1,
                ..Limits::PRODUCTION
            }
        )
        .unwrap_err()
        .contains("output bytes")
    );
    assert!(
        admit(
            &fixture,
            &Limits {
                source_file_bytes: 1,
                ..Limits::PRODUCTION
            }
        )
        .unwrap_err()
        .contains("project lock cannot be read")
    );
    assert!(admit(&fixture, &Limits::PRODUCTION).is_ok());
}

#[test]
fn session_unavailable_base_refuses_beside_disabled_graph() {
    let fixture = initialized();
    let path = fixture.catalog.join("core/backends/graphdb/config.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let graph = include_str!("../../../../../tests/fixtures/catalog/backends/graphdb.toml");
    fs::write(path, graph).unwrap();
    let port = fixture.port().unwrap();
    assert!(
        preview(&Fixture::new().project, &port, &["base".into()])
            .unwrap_err()
            .contains("not compiled")
    );
    let project = fixture.root.join("native-project");
    fs::create_dir(&project).unwrap();
    let compiled = BTreeSet::from(["ladybug".to_owned()]);
    let proposal = preview(
        &project,
        &port.with_compiled_backends(compiled.clone()),
        &["base".into()],
    )
    .unwrap();
    apply(&project, &proposal).unwrap();
    with_trust(&project, |trust| {
        let load = || {
            SessionPreferences::load(
                &fixture.root.join("user"),
                Some(&project),
                Some(&fixture.root),
                trust,
                &Limits::PRODUCTION,
            )
            .unwrap()
        };
        assert!(matches!(
            load().admit_defaults(trust, &BTreeSet::new(), &Limits::PRODUCTION),
            Err(AdmissionError::BackendNotCompiled { backend, .. }) if backend == "ladybug"
        ));
        assert!(
            load()
                .admit_defaults(trust, &compiled, &Limits::PRODUCTION)
                .is_ok()
        );
    });
}

#[test]
fn session_defaults_preserve_four_layers_and_snapshot() {
    use crate::settings::resolve;
    use maestro_settings::parse_flags;
    let fixture = initialized();
    let user = fixture.root.join("user");
    fs::create_dir(&user).unwrap();
    fs::write(
        user.join("preferences.toml"),
        "schema = 'maestro-preferences/1'\ntone = 'detailed'\n",
    )
    .unwrap();
    let config = fixture.project.join(".maestro/config.toml");
    fs::write(
        &config,
        "schema = 'maestro-preferences/1'\ntone = 'brief'\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let admitted = admit(&fixture, &Limits::PRODUCTION).unwrap();
    let registry = admitted.registry().unwrap();
    let layers = admitted.layers(&registry, &Limits::PRODUCTION).unwrap();
    let flags = parse_flags(&registry, &["tone=normal".to_owned()]).unwrap();
    assert_eq!(
        resolve(
            &registry,
            &maestro_settings::resolve(&registry, &layers, &flags)
        )
        .text("tone"),
        Some("normal")
    );
    assert_eq!(
        resolve(
            &registry,
            &maestro_settings::resolve(&registry, &layers, &[])
        )
        .text("tone"),
        Some("brief")
    );
    assert_eq!(
        resolve(
            &registry,
            &maestro_settings::resolve(
                &registry,
                &maestro_settings::Layers {
                    project: None,
                    ..layers.clone()
                },
                &[]
            )
        )
        .text("tone"),
        Some("detailed")
    );
    assert_eq!(
        resolve(
            &registry,
            &maestro_settings::resolve(&registry, &layers, &[])
        )
        .text("language"),
        Some("fr")
    );
    fs::write(
        &config,
        "schema = 'maestro-preferences/1'\nlanguage = 'ja'\n",
    )
    .unwrap();
    fs::write(
        fixture.project.join(".maestro/authoring.lock.json"),
        b"changed lock",
    )
    .unwrap();
    assert_eq!(
        resolve(
            &registry,
            &maestro_settings::resolve(
                &registry,
                &admitted.layers(&registry, &Limits::PRODUCTION).unwrap(),
                &[]
            )
        )
        .text("language"),
        Some("fr")
    );
    assert!(admit(&fixture, &Limits::PRODUCTION).is_err());
}

#[test]
fn source_and_session_default_secret_literals_are_redacted() {
    use crate::files::{FileInput, tests::support as writer};
    const SENTINEL: &str = "c60-synthetic-resolved-value-never-output";
    for key in ["credential", "tone"] {
        let defaults = format!("schema = 'maestro-preferences/1'\n{key} = '{SENTINEL}'\n");
        let fixture = Fixture::new();
        fs::create_dir(fixture.catalog.join("settings")).unwrap();
        fs::write(fixture.catalog.join("settings/defaults.toml"), &defaults).unwrap();
        let error = fixture.port().unwrap_err();
        assert!(!error.contains(SENTINEL), "{error}");
        assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema":"maestro-authoring-lock/3", "defaults":defaults,
            "backend_types":[], "files":[]
        }))
        .unwrap();
        let plan = writer::preview(
            &fixture.project,
            [FileInput::new(".maestro/authoring.lock.json", bytes)],
        )
        .unwrap();
        writer::apply(&fixture.project, &plan).unwrap();
        let error = admit(&fixture, &Limits::PRODUCTION).unwrap_err();
        assert!(error.contains(key), "{error}");
        assert!(!error.contains(SENTINEL), "{error}");
        assert_eq!(
            fs::read_dir(fixture.project.join(".maestro"))
                .unwrap()
                .count(),
            1
        );
    }
}

#[cfg(unix)]
#[test]
fn untrusted_unreadable_lock_refuses_before_read() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = initialized();
    let lock = fixture.project.join(".maestro/authoring.lock.json");
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o000)).unwrap();
    let captured = snapshot(&fixture);
    let boundaries = TrustBoundaries::new(&fixture.root, &[]).unwrap();
    let denied = CheckedTrust::new(&NoWorkspaceTrust, &boundaries);
    let result = captured.admit_defaults(&denied, &BTreeSet::new(), &Limits::PRODUCTION);
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o600)).unwrap();
    let error = result.unwrap_err().to_string();
    assert!(error.contains("trusted containing root"), "{error}");
}

#[test]
fn session_lock_output_byte_limit_refuses() {
    use crate::files::{FileInput, digest, tests::support as writer};
    let fixture = Fixture::new();
    let bytes = vec![b'a'; 5000];
    let lock = serde_json::to_vec(&serde_json::json!({
        "schema":"maestro-authoring-lock/3", "defaults":"schema = 'maestro-preferences/1'\n",
        "backend_types":[], "files":[{"path":"large.md", "sha256":digest(&bytes)}]
    }))
    .unwrap();
    let max = u64::try_from(lock.len()).unwrap() + 1;
    let plan = writer::preview(
        &fixture.project,
        [
            FileInput::new("large.md", bytes),
            FileInput::new(".maestro/authoring.lock.json", lock),
        ],
    )
    .unwrap();
    writer::apply(&fixture.project, &plan).unwrap();
    assert!(admit(&fixture, &Limits::PRODUCTION).is_ok());
    let error = admit(
        &fixture,
        &Limits {
            source_file_bytes: max,
            ..Limits::PRODUCTION
        },
    )
    .unwrap_err();
    assert!(error.contains("output exceeds byte limit"), "{error}");
}

#[cfg(unix)]
#[test]
fn unsafe_project_lock_refuses_after_trust() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = initialized();
    let lock = fixture.project.join(".maestro/authoring.lock.json");
    assert!(admit(&fixture, &Limits::PRODUCTION).is_ok());
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o666)).unwrap();
    let error = admit(&fixture, &Limits::PRODUCTION).unwrap_err();
    assert!(error.contains("other-writable"), "{error}");
    fs::set_permissions(lock, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(admit(&fixture, &Limits::PRODUCTION).is_ok());
}

#[test]
fn loaded_project_lock_cannot_bypass_admission() {
    let fixture = initialized();
    assert!(snapshot(&fixture).registry().is_err());
    assert!(
        admit(&fixture, &Limits::PRODUCTION)
            .unwrap()
            .registry()
            .is_ok()
    );
    assert!(snapshot(&Fixture::new()).registry().is_ok());
}

#[test]
fn session_aggregate_byte_boundary_counts_lock_and_outputs() {
    let fixture = initialized();
    let bytes = fs::read(fixture.project.join(".maestro/authoring.lock.json")).unwrap();
    let lock: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let output_bytes: u64 = lock["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            fs::metadata(fixture.project.join(entry["path"].as_str().unwrap()))
                .unwrap()
                .len()
        })
        .sum();
    let total = output_bytes + u64::try_from(bytes.len()).unwrap();
    let limits = Limits {
        archive_total_bytes: total - 1,
        ..Limits::PRODUCTION
    };
    let error = admit(&fixture, &limits).unwrap_err();
    assert!(error.contains("output bytes exceed limit"), "{error}");
    assert!(
        admit(
            &fixture,
            &Limits {
                archive_total_bytes: total,
                ..limits
            }
        )
        .is_ok()
    );
}
