//! Owner-local data inventories through the shared bootstrap composition.
use super::{
    super::{AreaInventories, PresetPort},
    support::{apply, preview},
};
use crate::files::digest;
use maestro_test_scratch::scratch_directory;
use std::{fs, path::PathBuf};

/// Synthetic catalog and empty project with automatic scratch cleanup.
pub(super) struct Fixture {
    pub(super) root: PathBuf,
    pub(super) catalog: PathBuf,
    pub(super) project: PathBuf,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let root = scratch_directory().unwrap();
        let catalog = root.join("catalog");
        let project = root.join("project");
        fs::create_dir(&project).unwrap();
        super::support::copy_catalog(&catalog);
        Self {
            root,
            catalog,
            project,
        }
    }

    pub(super) fn port(&self) -> Result<AreaInventories, String> {
        super::support::checked_port(&self.catalog)
    }

    pub(super) fn edit(&self, path: &str, old: &str, new: &str) {
        let path = self.catalog.join(path);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(old));
        fs::write(path, text.replace(old, new)).unwrap();
    }

    pub(super) fn refuses(&self, names: &[String], message: &str) {
        let error = self
            .port()
            .and_then(|port| preview(&self.project, &port, names))
            .unwrap_err();
        assert!(error.contains(message), "{error}");
        assert_eq!(fs::read_dir(&self.project).unwrap().count(), 0);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.root));
    }
}

#[test]
fn selected_owner_inventory_accepts() {
    let fixture = Fixture::new();
    let port = fixture.port().unwrap();
    for names in [vec!["base".into()], vec!["base".into(), "rust".into()]] {
        let resolved = port.resolve(&names).unwrap();
        let files: Vec<_> = resolved
            .iter()
            .flat_map(|preset| preset.files.keys())
            .collect();
        assert_eq!(
            files.len(),
            names.len(),
            "common inventory must be included once"
        );
        let proposal = preview(&fixture.project, &port, &names).unwrap();
        assert_eq!(proposal.prerequisites.len(), names.len());
        assert_eq!(proposal.bindings.len(), names.len());
        assert!(
            proposal
                .bindings
                .contains(&"tool:common/inspect".to_owned())
        );
        assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    }
    let proposal = preview(&fixture.project, &port, &["base".into(), "rust".into()]).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    assert_eq!(
        fs::read(fixture.project.join(".maestro/recipes.json")).unwrap(),
        b"{\"recipes\":[]}\n"
    );
    let lock: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.project.join(".maestro/authoring.lock.json")).unwrap(),
    )
    .unwrap();
    let paths: Vec<_> = lock["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| source["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths.len(), 6);
    assert!(paths.contains(&"bootstrap/base.toml"));
    assert!(paths.contains(&"languages/rust/bootstrap/starter.toml"));
}

#[test]
fn unselected_inventory_refuses() {
    let fixture = Fixture::new();
    assert!(fixture.port().unwrap().resolve(&["rust".into()]).is_ok());
    fixture.edit(
        "presets/rust.toml",
        r#"requires = ["language:rust"]"#,
        "requires = []",
    );
    fixture.refuses(&["rust".into()], "unselected area: rust");
    assert!(fixture.port().unwrap().resolve(&["base".into()]).is_ok());
}

#[test]
fn unknown_inventory_refuses() {
    let fixture = Fixture::new();
    assert!(fixture.port().unwrap().resolve(&["base".into()]).is_ok());
    fixture.edit("presets/base.toml", "common/base", "common/missing");
    fixture.refuses(&["base".into()], "bootstrap-inventory:common/missing");
}

#[test]
fn inventory_escape_refuses() {
    for field in ["source", "output"] {
        for escape in [
            "../outside",
            "/outside",
            "nested/../../outside",
            "C:\\outside",
        ] {
            let fixture = Fixture::new();
            assert!(fixture.port().unwrap().resolve(&["base".into()]).is_ok());
            let old = if field == "source" {
                "instructions.md"
            } else {
                ".github/copilot-instructions.md"
            };
            fixture.edit(
                "bootstrap/base.toml",
                &format!("{field} = \"{old}\""),
                &format!("{field} = '{escape}'"),
            );
            fixture.refuses(&["base".into()], "unsafe preset file path");
        }
    }
}

#[test]
fn identical_output_collision_refuses() {
    let fixture = Fixture::new();
    assert!(fixture.port().unwrap().resolve(&["rust".into()]).is_ok());
    let bytes = fs::read(fixture.catalog.join("bootstrap/base/files/instructions.md")).unwrap();
    fs::write(
        fixture
            .catalog
            .join("languages/rust/bootstrap/starter/files/recipes.json"),
        &bytes,
    )
    .unwrap();
    fixture.edit(
        "languages/rust/bootstrap/starter.toml",
        ".maestro/recipes.json",
        ".github/copilot-instructions.md",
    );
    let inventory = fixture
        .catalog
        .join("languages/rust/bootstrap/starter.toml");
    let text = fs::read_to_string(&inventory).unwrap();
    let old = text
        .lines()
        .find(|line| line.starts_with("sha256 ="))
        .unwrap();
    fs::write(
        &inventory,
        text.replace(old, &format!("sha256 = \"{}\"", digest(&bytes))),
    )
    .unwrap();
    for names in [vec!["rust".into()], vec!["base".into(), "rust".into()]] {
        fixture.refuses(&names, "file collision: .github/copilot-instructions.md");
    }
}

#[test]
fn changed_inventory_input_requires_fresh_preview() {
    for path in [
        "presets/base.toml",
        "bootstrap/base.toml",
        "bootstrap/base/files/instructions.md",
    ] {
        let fixture = Fixture::new();
        let port = fixture.port().unwrap();
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
        fs::write(fixture.catalog.join(path), original).unwrap();
        let fresh = preview(&fixture.project, &port, &["base".into()]).unwrap();
        apply(&fixture.project, &fresh).unwrap();
    }
}

#[test]
fn inventory_digest_name_tools_and_strict_fields_refuse() {
    for (old, new, message) in [
        ("name = \"base\"", "name = \"wrong\"", "file stem"),
        ("sha256:", "wrong:", "inventory digest mismatch"),
        (
            "tools = [\"sh\"]",
            "tools = [\"sh --install\"]",
            "single top-level name",
        ),
        (
            "tools = [\"sh\"]",
            "tools = [\"../sh\"]",
            "unsafe preset file path",
        ),
        (
            r#"tools = ["sh"]"#,
            "tools = [\"sh\"]\nextra = true",
            "unknown",
        ),
        (
            r#"source = "instructions.md""#,
            "extra = true\nsource = \"instructions.md\"",
            "unknown",
        ),
    ] {
        let fixture = Fixture::new();
        assert!(fixture.port().unwrap().resolve(&["base".into()]).is_ok());
        fixture.edit("bootstrap/base.toml", old, new);
        fixture.refuses(&["base".into()], message);
    }
}

#[test]
fn duplicate_output_inside_inventory_refuses() {
    let fixture = Fixture::new();
    let path = fixture.catalog.join("bootstrap/base.toml");
    let text = fs::read_to_string(&path).unwrap();
    let entry = text
        .split_once("[[files]]")
        .unwrap()
        .1
        .split_once("[metadata]")
        .unwrap()
        .0;
    fs::write(
        path,
        text.replace("[metadata]", &format!("[[files]]{entry}\n[metadata]")),
    )
    .unwrap();
    fixture.refuses(&["base".into()], "duplicate inventory source/output");
}

#[test]
fn preset_inventory_selector_grammar_refuses() {
    for selector in [
        "../base",
        "common/base/extra",
        "common",
        "common/",
        "Common/base",
        "common/Base",
        "common/base.toml",
    ] {
        let fixture = Fixture::new();
        fixture.edit("presets/base.toml", "common/base", selector);
        fixture.refuses(&["base".into()], "area/inventory");
    }
}

#[cfg(unix)]
#[test]
fn owner_inventory_links_refuse_without_writes() {
    use std::os::unix::fs::symlink;
    for path in [
        "bootstrap/base.toml",
        "bootstrap/base/files/instructions.md",
        "bootstrap/base/files",
    ] {
        let fixture = Fixture::new();
        let target = fixture.catalog.join(path);
        let outside = fixture.root.join("outside");
        fs::rename(&target, &outside).unwrap();
        symlink(&outside, target).unwrap();
        fixture.refuses(&["base".into()], "links and special files");
    }
}

#[test]
fn inventory_aggregate_bounds_have_passing_neighbours() {
    use crate::limits::Limits;
    let fixture = Fixture::new();
    let bytes = [
        "presets/base.toml",
        "bootstrap/base.toml",
        "bootstrap/base/files/instructions.md",
    ]
    .iter()
    .map(|path| fs::metadata(fixture.catalog.join(path)).unwrap().len())
    .sum::<u64>();
    let limits = Limits {
        archive_entries: 3,
        archive_total_bytes: bytes,
        ..Limits::PRODUCTION
    };
    assert!(
        fixture
            .port()
            .unwrap()
            .with_limits(limits)
            .resolve(&["base".into()])
            .is_ok()
    );
    for (limits, message) in [
        (
            Limits {
                archive_entries: 2,
                ..limits
            },
            "source count exceeds limit",
        ),
        (
            Limits {
                archive_total_bytes: bytes - 1,
                ..limits
            },
            "source bytes exceed limit",
        ),
    ] {
        let error = fixture
            .port()
            .unwrap()
            .with_limits(limits)
            .resolve(&["base".into()])
            .unwrap_err();
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn owner_inventory_generated_json_remains_strict() {
    let fixture = Fixture::new();
    let invalid = br#"{"recipes":[],"recipes":[]}"#;
    fs::write(
        fixture
            .catalog
            .join("languages/rust/bootstrap/starter/files/recipes.json"),
        invalid,
    )
    .unwrap();
    let inventory = fixture
        .catalog
        .join("languages/rust/bootstrap/starter.toml");
    let text = fs::read_to_string(&inventory).unwrap();
    let old = text
        .lines()
        .find(|line| line.starts_with("sha256 ="))
        .unwrap();
    fs::write(
        &inventory,
        text.replace(old, &format!("sha256 = \"{}\"", digest(invalid))),
    )
    .unwrap();
    fixture.refuses(&["rust".into()], "duplicate object key");
}

#[test]
fn owner_inventory_paths_and_requirements_are_data() {
    let fixture = Fixture::new();
    fs::rename(
        fixture.catalog.join("languages/rust"),
        fixture.catalog.join("languages/python"),
    )
    .unwrap();
    fixture.edit(
        "languages/python/package.toml",
        r#"name = "rust""#,
        r#"name = "python""#,
    );
    fixture.edit("presets/rust.toml", "language:rust", "language:python");
    fixture.edit("presets/rust.toml", "rust/starter", "python/starter");
    let proposal = preview(&fixture.project, &fixture.port().unwrap(), &["rust".into()]).unwrap();
    assert_eq!(
        proposal.bindings,
        ["tool:common/inspect", "tool:rust/check"]
    );
    assert_eq!(
        proposal
            .prerequisites
            .iter()
            .map(|requirement| requirement.tool.as_str())
            .collect::<Vec<_>>(),
        ["cargo", "sh"]
    );
    fixture.edit(
        "languages/python/package.toml",
        r#"name = "python""#,
        r#"name = "wrong""#,
    );
    fixture.refuses(&["rust".into()], "area name");
}

#[test]
fn preset_loader_reuses_registered_shape_and_metadata() {
    for (old, new, message) in [
        ("name = \"base\"", "name = \"other\"", "file stem"),
        (
            "description = \"Synthetic starter\"",
            "extra = true",
            "unknown key",
        ),
        (
            "schema = \"maestro-source/2\"",
            "schema = \"maestro-source/1\"",
            "schema",
        ),
        ("common/base", "Common/base", "area/inventory"),
    ] {
        let fixture = Fixture::new();
        fixture.edit("presets/base.toml", old, new);
        fixture.refuses(&["base".into()], message);
    }
}

#[test]
fn inventory_file_byte_bound_refuses_one_past() {
    use crate::limits::Limits;
    let fixture = Fixture::new();
    let file = fixture.catalog.join("bootstrap/base/files/instructions.md");
    let bytes = vec![b'x'; usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap()];
    fs::write(&file, &bytes).unwrap();
    let inventory = fixture.catalog.join("bootstrap/base.toml");
    let text = fs::read_to_string(&inventory).unwrap();
    let old = text
        .lines()
        .find(|line| line.starts_with("sha256 ="))
        .unwrap();
    fs::write(
        &inventory,
        text.replace(old, &format!("sha256 = \"{}\"", digest(&bytes))),
    )
    .unwrap();
    assert!(fixture.port().unwrap().resolve(&["base".into()]).is_ok());
    let mut oversized = bytes;
    oversized.push(b'x');
    fs::write(file, oversized).unwrap();
    fixture.refuses(&["base".into()], "larger than");
}

#[test]
fn preset_names_are_functional_not_paths() {
    let fixture = Fixture::new();
    let text = fs::read_to_string(fixture.catalog.join("presets/base.toml")).unwrap();
    fs::write(
        fixture.catalog.join("presets/Bad.toml"),
        text.replace(r#"name = "base""#, r#"name = "Bad""#),
    )
    .unwrap();
    fixture.refuses(&["Bad".into()], "functional name");
    fixture.refuses(&["../base".into()], "functional name");
}

#[test]
fn distinct_sources_same_inventory_output_refuse() {
    let fixture = Fixture::new();
    let manifest = fixture.catalog.join("bootstrap/base.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    let entry = text
        .split_once("[[files]]")
        .unwrap()
        .1
        .split_once("[metadata]")
        .unwrap()
        .0;
    let second = entry.replace("source = \"instructions.md\"", "source = \"second.md\"");
    fs::copy(
        fixture.catalog.join("bootstrap/base/files/instructions.md"),
        fixture.catalog.join("bootstrap/base/files/second.md"),
    )
    .unwrap();
    fs::write(
        &manifest,
        text.replace("[metadata]", &format!("[[files]]{second}\n[metadata]")),
    )
    .unwrap();
    fixture.refuses(
        &["base".into()],
        "file collision: .github/copilot-instructions.md",
    );
    let text = fs::read_to_string(&manifest).unwrap().replacen(
        "output = \".github/copilot-instructions.md\"",
        "output = \"second.md\"",
        1,
    );
    fs::write(manifest, text).unwrap();
    let resolved = fixture.port().unwrap().resolve(&["base".into()]).unwrap();
    assert_eq!(resolved[0].files.len(), 2);
    assert_eq!(resolved[0].source_files.len(), 4);
    assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
}
