//! Complete source locks, stale-input refusals and owned user-byte preservation.
use super::{
    inventory::Fixture,
    support::{apply, preview},
};
use crate::files::{digest, tests::support::remove};
use std::fs;

/// Exact selected closure, independent of the production capture loop.
const EXPECTED: [(&str, &str, Option<&str>); 17] = [
    (
        "bootstrap/base.toml",
        "bootstrap-inventory:common/base",
        None,
    ),
    (
        "bootstrap/base/files/instructions.md",
        "bootstrap-inventory:common/base",
        None,
    ),
    (
        "core/agents/maestro.agent.md",
        "agent:core/maestro",
        Some("2.3.4"),
    ),
    (
        "core/agents/maestro.maestro.toml",
        "agent:core/maestro",
        Some("2.3.4"),
    ),
    ("core/package.toml", "package:core", Some("1.2.3")),
    (
        "languages/rust/bootstrap/starter.toml",
        "bootstrap-inventory:rust/starter",
        None,
    ),
    (
        "languages/rust/bootstrap/starter/files/recipes.json",
        "bootstrap-inventory:rust/starter",
        None,
    ),
    (
        "languages/rust/instructions/rules.instructions.md",
        "instructions:rust/rules",
        None,
    ),
    (
        "languages/rust/instructions/rules.maestro.toml",
        "instructions:rust/rules",
        None,
    ),
    (
        "languages/rust/package.toml",
        "language:rust",
        Some("1.2.3"),
    ),
    (
        "languages/rust/profiles/quality/default.toml",
        "quality-profile:rust/default",
        None,
    ),
    ("package.toml", "package:common", Some("1.2.3")),
    ("presets/base.toml", "preset:base", None),
    ("presets/rust.toml", "preset:rust", None),
    (
        "standards/quality/package.toml",
        "standard:quality",
        Some("1.2.3"),
    ),
    (
        "standards/quality/profiles/quality/baseline.toml",
        "quality-profile:quality/baseline",
        None,
    ),
    (
        "standards/security/package.toml",
        "standard:security",
        Some("1.2.3"),
    ),
];

#[test]
fn old_authoring_lock_requires_preview() {
    let fixture = Fixture::new();
    let port = fixture.port().unwrap();
    let proposal = preview(&fixture.project, &port, &["base".into()]).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    assert!(
        preview(&fixture.project, &port, &["base".into()])
            .unwrap()
            .plan
            .is_applied()
    );
    let path = fixture.project.join(".maestro/authoring.lock.json");
    let original = fs::read(&path).unwrap();
    fs::write(
        &path,
        br#"{"schema":"maestro-authoring-lock/1","files":[],"sources":[]}"#,
    )
    .unwrap();
    let error = preview(&fixture.project, &port, &["base".into()]).unwrap_err();
    assert!(
        error.contains("fresh preview") && error.contains("maestro-authoring-lock/1"),
        "{error}"
    );
    assert!(remove(&fixture.project, proposal.plan.id()).is_err());
    assert_eq!(
        fs::read(&path).unwrap(),
        br#"{"schema":"maestro-authoring-lock/1","files":[],"sources":[]}"#
    );
    fs::write(&path, original).unwrap();
    let mut incomplete: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    incomplete["sources"] = serde_json::json!([]);
    let changed = serde_json::to_vec(&incomplete).unwrap();
    fs::write(&path, &changed).unwrap();
    let error = preview(&fixture.project, &port, &["base".into()]).unwrap_err();
    assert!(error.contains("preview again"), "{error}");
    assert!(remove(&fixture.project, proposal.plan.id()).is_err());
    assert_eq!(fs::read(&path).unwrap(), changed);
}

#[test]
fn every_selected_input_is_locked() {
    let fixture = Fixture::new();
    fixture.edit(
        "core/agents/maestro.maestro.toml",
        "maturity = \"reviewed\"",
        "version = \"2.3.4\"\nmaturity = \"reviewed\"",
    );
    let proposal = preview(
        &fixture.project,
        &fixture.port().unwrap(),
        &["rust".into(), "base".into()],
    )
    .unwrap();
    apply(&fixture.project, &proposal).unwrap();
    let lock: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.project.join(".maestro/authoring.lock.json")).unwrap(),
    )
    .unwrap();
    let sources = lock["sources"].as_array().unwrap();
    assert_eq!(
        sources.len(),
        EXPECTED.len(),
        "complete selected source closure"
    );
    for (source, (path, id, revision)) in sources.iter().zip(EXPECTED) {
        assert_eq!(source["path"], path);
        assert_eq!(source["id"], id, "{path}");
        assert_eq!(source["revision"], serde_json::json!(revision), "{path}");
        assert_eq!(
            source["sha256"],
            digest(&fs::read(fixture.catalog.join(path)).unwrap())
        );
    }
    let areas = [
        "language:rust",
        "package:common",
        "package:core",
        "standard:quality",
        "standard:security",
    ];
    assert_eq!(lock["areas"], serde_json::json!(areas));
    let project: toml::Table =
        toml::from_str(&fs::read_to_string(fixture.project.join(".maestro/project.toml")).unwrap())
            .unwrap();
    assert_eq!(
        project["areas"].as_array().unwrap(),
        &areas.map(|area| toml::Value::String(area.into()))
    );
    assert_eq!(
        project["presets"].as_array().unwrap(),
        &[
            toml::Value::String("preset:base".into()),
            toml::Value::String("preset:rust".into())
        ]
    );
    let reordered = preview(
        &fixture.project,
        &fixture.port().unwrap(),
        &["base".into(), "rust".into()],
    )
    .unwrap();
    assert!(reordered.plan.is_applied(), "stable selection order");
}

#[test]
fn changed_source_path_requires_preview() {
    for path in [
        "package.toml",
        "core/agents/maestro.agent.md",
        "core/agents/maestro.maestro.toml",
        "languages/rust/package.toml",
        "standards/quality/package.toml",
        "presets/rust.toml",
        "languages/rust/bootstrap/starter.toml",
        "languages/rust/bootstrap/starter/files/recipes.json",
    ] {
        for rename in [false, true] {
            let fixture = Fixture::new();
            let port = fixture.port().unwrap();
            let proposal = preview(&fixture.project, &port, &["rust".into()]).unwrap();
            let source = fixture.catalog.join(path);
            let original = fs::read(&source).unwrap();
            if rename {
                fs::rename(&source, source.with_extension("moved")).unwrap();
            } else {
                let mut edited = original.clone();
                edited.push(b'\n');
                fs::write(&source, edited).unwrap();
            }
            let error = apply(&fixture.project, &proposal).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("input changed; run preview again"),
                "{path}: {error}"
            );
            assert_eq!(
                fs::read_dir(&fixture.project).unwrap().count(),
                0,
                "zero writes: {path}"
            );
            if rename {
                fs::rename(source.with_extension("moved"), &source).unwrap();
            } else {
                fs::write(source, original).unwrap();
            }
            let fresh =
                preview(&fixture.project, &fixture.port().unwrap(), &["rust".into()]).unwrap();
            apply(&fixture.project, &fresh).unwrap();
            let output = fixture.project.join(".github/copilot-instructions.md");
            fs::write(&output, b"user bytes").unwrap();
            assert!(preview(&fixture.project, &fixture.port().unwrap(), &["rust".into()]).is_err());
            assert!(remove(&fixture.project, fresh.plan.id()).is_err());
            assert_eq!(fs::read(output).unwrap(), b"user bytes");
        }
    }
}

#[test]
fn selected_resource_requires_its_area_descriptor() {
    let fixture = Fixture::new();
    let path = fixture.catalog.join("presets/base.toml");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        format!("{original}\nrequires = [\"bootstrap-inventory:rust/starter\"]\n"),
    )
    .unwrap();
    let error = preview(&fixture.project, &fixture.port().unwrap(), &["base".into()]).unwrap_err();
    assert!(
        error.contains("bootstrap-inventory:rust/starter requires language:rust in the selection"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    fs::write(
        path,
        format!(
            "{original}\nrequires = [\"bootstrap-inventory:rust/starter\", \"language:rust\"]\n"
        ),
    )
    .unwrap();
    let proposal = preview(&fixture.project, &fixture.port().unwrap(), &["base".into()]).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    let lock: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.project.join(".maestro/authoring.lock.json")).unwrap(),
    )
    .unwrap();
    assert!(lock["sources"].as_array().unwrap().iter().any(|source| {
        source["id"] == "language:rust" && source["path"] == "languages/rust/package.toml"
    }));
}

#[test]
fn genuine_committed_old_v2_lock_refuses_rebind() {
    use crate::files::{FileInput, tests::support as writer};
    let fixture = Fixture::new();
    let port = fixture.port().unwrap();
    let proposal = preview(&fixture.project, &port, &["base".into()]).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    let guide_path = ".github/copilot-instructions.md";
    let project_path = ".maestro/project.toml";
    let lock_path = ".maestro/authoring.lock.json";
    let guide = fs::read(fixture.project.join(guide_path)).unwrap();
    let mut descriptor: toml::Table =
        toml::from_str(&fs::read_to_string(fixture.project.join(project_path)).unwrap()).unwrap();
    descriptor.remove("areas");
    let descriptor_bytes = toml::to_string(&descriptor).unwrap().into_bytes();
    let mut lock: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.project.join(lock_path)).unwrap()).unwrap();
    lock.as_object_mut().unwrap().remove("areas");
    let sources = lock["sources"].as_array_mut().unwrap();
    sources.retain(|source| {
        matches!(
            source["path"].as_str().unwrap(),
            "presets/base.toml" | "bootstrap/base.toml" | "bootstrap/base/files/instructions.md"
        )
    });
    for source in sources {
        source.as_object_mut().unwrap().remove("id");
        source.as_object_mut().unwrap().remove("revision");
    }
    for file in lock["files"].as_array_mut().unwrap() {
        if file["path"] == project_path {
            file["sha256"] = serde_json::json!(digest(&descriptor_bytes));
        }
    }
    let lock_bytes = serde_json::to_vec_pretty(&lock).unwrap();
    remove(&fixture.project, proposal.plan.id()).unwrap();
    let old = writer::preview(
        &fixture.project,
        [
            FileInput::new(guide_path, guide.clone()),
            FileInput::new(project_path, descriptor_bytes.clone()),
            FileInput::new(lock_path, lock_bytes.clone()),
        ],
    )
    .unwrap();
    writer::apply(&fixture.project, &old).unwrap();
    let error = preview(&fixture.project, &port, &["base".into()]).unwrap_err();
    assert!(error.contains("preview again"), "{error}");
    assert_eq!(fs::read(fixture.project.join(guide_path)).unwrap(), guide);
    assert_eq!(
        fs::read(fixture.project.join(project_path)).unwrap(),
        descriptor_bytes
    );
    assert_eq!(
        fs::read(fixture.project.join(lock_path)).unwrap(),
        lock_bytes
    );
    fs::write(fixture.project.join(guide_path), b"user bytes").unwrap();
    assert!(preview(&fixture.project, &port, &["base".into()]).is_err());
    assert!(remove(&fixture.project, old.id()).is_err());
    assert_eq!(
        fs::read(fixture.project.join(guide_path)).unwrap(),
        b"user bytes"
    );
    assert_eq!(
        fs::read(fixture.project.join(lock_path)).unwrap(),
        lock_bytes
    );
}

#[test]
fn registered_checked_config_is_locked_and_revalidated() {
    use crate::bootstrap::AreaInventories;
    use crate::source::{
        Field, FieldType, Format, KindDescriptor, Known, Layout, Maturity, MetadataPlace, Scope,
        builtin, frozen_rows,
    };
    let fixture = Fixture::new();
    let path = "core/checks/settings.toml";
    fs::create_dir_all(fixture.catalog.join("core/checks")).unwrap();
    fs::write(
        fixture.catalog.join(path),
        "name = \"settings\"\n\
         [metadata]\n\
         schema = \"maestro-source/2\"\n\
         maturity = \"reviewed\"\n\
         version = \"9.8.7\"\n\
         rows = [\"owner.catalog\"]\n\
         workflows = [\"ctm-question\"]\n",
    )
    .unwrap();
    let preset_path = fixture.catalog.join("presets/base.toml");
    let preset = fs::read_to_string(&preset_path).unwrap();
    fs::write(
        preset_path,
        format!("{preset}\nrequires = [\"checked-config:core/settings\"]\n"),
    )
    .unwrap();
    let mut registry = builtin().unwrap();
    registry
        .register(KindDescriptor {
            kind: "checked-config".into(),
            version: 1,
            directory: "checks".into(),
            scopes: vec![Scope::Core],
            layout: Layout::Files {
                suffix: ".toml".into(),
                folders: vec![String::new()],
            },
            format: Format::Toml,
            metadata: MetadataPlace::Table {
                key: "metadata".into(),
            },
            name_field: Some("name".into()),
            fields: vec![Field::required("name", FieldType::Text)],
            body: false,
            requires: vec![],
            lifecycle: Maturity::DECLARABLE.to_vec(),
            closure_root: false,
            required: None,
            hook: None,
        })
        .unwrap();
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let port = AreaInventories::new(
        &fixture.catalog,
        registry,
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
    )
    .unwrap();
    let proposal = preview(&fixture.project, &port, &["base".into()]).unwrap();
    let original = fs::read(fixture.catalog.join(path)).unwrap();
    let mut changed = original.clone();
    changed.push(b'\n');
    fs::write(fixture.catalog.join(path), changed).unwrap();
    assert!(
        apply(&fixture.project, &proposal)
            .unwrap_err()
            .to_string()
            .contains("input changed; run preview again")
    );
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    fs::write(fixture.catalog.join(path), &original).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    let lock: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.project.join(".maestro/authoring.lock.json")).unwrap(),
    )
    .unwrap();
    let source = lock["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["path"] == path)
        .unwrap();
    assert_eq!(source["id"], "checked-config:core/settings");
    assert_eq!(source["revision"], "9.8.7");
    assert_eq!(source["sha256"], digest(&original));
}

#[test]
fn requires_only_inventory_still_locks_its_assets() {
    let fixture = Fixture::new();
    fixture.edit(
        "presets/base.toml",
        "templates = [\"common/base\"]",
        "templates = []",
    );
    let path = fixture.catalog.join("presets/base.toml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        path,
        format!("{text}\nrequires = [\"bootstrap-inventory:common/base\"]\n"),
    )
    .unwrap();
    let port = fixture.port().unwrap();
    let proposal = preview(&fixture.project, &port, &["base".into()]).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    assert!(
        !fixture
            .project
            .join(".github/copilot-instructions.md")
            .exists()
    );
    let lock: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.project.join(".maestro/authoring.lock.json")).unwrap(),
    )
    .unwrap();
    let sources = lock["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 9);
    assert!(sources.iter().any(
        |source| source["path"] == "bootstrap/base/files/instructions.md"
            && source["id"] == "bootstrap-inventory:common/base"
    ));
    fs::write(
        fixture.catalog.join("bootstrap/base/files/instructions.md"),
        b"changed",
    )
    .unwrap();
    assert!(
        apply(&fixture.project, &proposal)
            .unwrap_err()
            .to_string()
            .contains("input changed; run preview again")
    );
}
