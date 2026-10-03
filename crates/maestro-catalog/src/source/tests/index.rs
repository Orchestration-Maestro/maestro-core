//! Public pinned source navigation and exact generated-byte drift neighbours.

use super::{area_packages::package_source, area_support::folder, support::MemoryTree};
use crate::{
    files::digest,
    limits::Limits,
    source::{
        BY_TYPE_PATH, Entry, INDEX_PATH, Known, Layout, SourceTree, builtin, frozen_rows,
        index::{CatalogIndex, generate},
    },
};
use serde_json::Value;
use std::{cell::RefCell, collections::BTreeSet, io};

/// Render only the public source selected by the caller, never another overlay.
fn render(public: &dyn SourceTree) -> CatalogIndex {
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    generate(
        public,
        &builtin().unwrap(),
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
    )
    .unwrap()
}

#[test]
fn catalog_index_is_deterministic() {
    let source = MemoryTree::valid();
    let first = render(&source);
    let second = render(&source);
    assert_eq!(first.index.as_bytes(), second.index.as_bytes());
    assert_eq!(first.by_type.as_bytes(), second.by_type.as_bytes());
    assert!(!first.index.contains('\r'));
    assert!(!first.by_type.contains('\r'));
    let document: Value = serde_json::from_str(&first.index).unwrap();
    assert_eq!(document["schema"], "maestro-catalog/index/1");
    let resources = document["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 6);
    let ids: Vec<_> = resources
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "agent:core/valid",
            "instructions:core/valid",
            "package:common",
            "package:core",
            "preset:knowledge-client",
            "skill:common/valid-skill"
        ]
    );
    for row in resources {
        assert!(first.by_type.contains(row["id"].as_str().unwrap()));
        for (path, pin) in row["sources"].as_object().unwrap() {
            assert_eq!(pin.as_str().unwrap(), digest(source.text(path).as_bytes()));
        }
    }
    assert_eq!(resources[3]["version"], "1.2.3");
    assert!(resources.iter().all(|row| row.get("installable").is_none()));
}

#[test]
fn stale_extra_index_row_refuses() {
    let rendered = render(&MemoryTree::valid());
    let mut document: Value = serde_json::from_str(&rendered.index).unwrap();
    for mutation in ["stale", "extra", "missing", "unknown"] {
        let mut changed = document.clone();
        match mutation {
            "stale" => {
                changed["resources"][0]["sources"]["core/agents/valid.agent.md"] =
                    "0".repeat(64).into();
            }
            "extra" => changed["resources"]
                .as_array_mut()
                .unwrap()
                .push(document["resources"][0].clone()),
            "missing" => {
                changed["resources"].as_array_mut().unwrap().pop();
            }
            _ => {
                changed["unexpected"] = true.into();
            }
        }
        assert!(
            rendered
                .verify(
                    Some(changed.to_string().as_bytes()),
                    Some(rendered.by_type.as_bytes())
                )
                .is_err(),
            "{mutation}"
        );
    }
    document["schema"] = "maestro-catalog/index/0".into();
    assert!(
        rendered
            .verify(
                Some(document.to_string().as_bytes()),
                Some(rendered.by_type.as_bytes())
            )
            .is_err()
    );
    assert!(
        rendered
            .verify(None, Some(rendered.by_type.as_bytes()))
            .is_err()
    );
    assert!(
        rendered
            .verify(Some(rendered.index.as_bytes()), None)
            .is_err()
    );
    assert!(
        rendered
            .verify(Some(rendered.index.as_bytes()), Some(b"stale view\n"))
            .is_err()
    );
    assert!(
        rendered
            .verify(
                Some(rendered.index.as_bytes()),
                Some(rendered.by_type.as_bytes())
            )
            .is_ok()
    );
}

#[test]
fn catalog_index_verify_committed_refuses_drift() {
    let source = MemoryTree::valid();
    let expected = render(&source);
    let committed = source
        .with(INDEX_PATH, &expected.index)
        .with(BY_TYPE_PATH, &expected.by_type);
    render(&committed).verify_committed().unwrap();
    let document: Value = serde_json::from_str(&expected.index).unwrap();
    for mutation in ["stale", "extra", "private"] {
        let mut changed = document.clone();
        match mutation {
            "stale" => changed["resources"][0]["version"] = "9.8.7".into(),
            "extra" => changed["resources"]
                .as_array_mut()
                .unwrap()
                .push(document["resources"][0].clone()),
            _ => changed["resources"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"id": "package:confidential"})),
        }
        let checked = render(&committed.clone().with(INDEX_PATH, &changed.to_string()));
        let refusal = checked.verify_committed().expect_err(mutation);
        assert_eq!(refusal.diagnostics.len(), 1, "{mutation}");
        let message = refusal.diagnostics[0].to_string();
        assert!(message.contains(INDEX_PATH), "{mutation}: {message}");
        assert!(
            message.contains("stale, extra or private rows"),
            "{mutation}: {message}"
        );
        assert!(message.contains("--view index"), "{mutation}: {message}");
        assert!(!message.contains("confidential"), "{mutation}: {message}");
    }
    for path in [INDEX_PATH, BY_TYPE_PATH] {
        let refusal = render(&committed.clone().without(path))
            .verify_committed()
            .unwrap_err();
        assert_eq!(refusal.diagnostics.len(), 1);
        let message = refusal.diagnostics[0].to_string();
        assert!(
            message.contains(path) && message.contains("missing"),
            "{message}"
        );
    }
    let refusal = render(&committed.with(BY_TYPE_PATH, "stale\n"))
        .verify_committed()
        .unwrap_err();
    assert_eq!(refusal.diagnostics.len(), 1);
    let message = refusal.diagnostics[0].to_string();
    assert!(message.contains(BY_TYPE_PATH) && message.contains("--view by-type"));
}

#[test]
fn catalog_index_unreadable_committed_views_fail_before_retention() {
    let source = MemoryTree::valid();
    let expected = render(&source);
    let committed = source
        .with(INDEX_PATH, &expected.index)
        .with(BY_TYPE_PATH, &expected.by_type);
    render(&committed).verify_committed().unwrap();
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    for path in [INDEX_PATH, BY_TYPE_PATH] {
        let refusal = generate(
            &committed.clone().with_unreadable(path),
            &builtin().unwrap(),
            &Limits::PRODUCTION,
            Known {
                rows: &rows,
                settings: &settings,
                today: 0,
            },
        )
        .unwrap_err();
        assert!(refusal.unreadable());
        let message = refusal.to_string();
        assert!(
            message.contains(path) && message.contains("cannot read"),
            "{message}"
        );
    }
}

#[test]
fn public_index_excludes_private() {
    let public = MemoryTree::valid();
    let private = MemoryTree::owned().with(
        "capabilities/practice/confidential/package.toml",
        &package_source("package", "confidential"),
    );
    // The selection retains two sources; only its public slot enters generation.
    let selection = (&public, &private);
    let rendered = render(selection.0);
    let private_digest = digest(
        selection
            .1
            .text("capabilities/practice/confidential/package.toml")
            .as_bytes(),
    );
    // This is a valid independent synthetic source, not a forged ID in public data.
    let private_view = render(selection.1);
    assert!(private_view.index.contains("package:confidential"));
    for bytes in [&rendered.index, &rendered.by_type] {
        assert!(!bytes.contains("confidential"));
        assert!(!bytes.contains("capabilities/practice/confidential/package.toml"));
        assert!(!bytes.contains(&private_digest));
    }
    let mut document: Value = serde_json::from_str(&rendered.index).unwrap();
    let private_document: Value = serde_json::from_str(&private_view.index).unwrap();
    let private_row = private_document["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "package:confidential")
        .unwrap();
    document["resources"]
        .as_array_mut()
        .unwrap()
        .push(private_row.clone());
    assert!(
        rendered
            .verify(
                Some(document.to_string().as_bytes()),
                Some(rendered.by_type.as_bytes())
            )
            .is_err()
    );
}

#[test]
fn catalog_index_keeps_exact_crlf_source_pins_and_lf_outputs() {
    let source = MemoryTree::valid();
    let path = "core/agents/valid.agent.md";
    let crlf = source.text(path).replace('\n', "\r\n");
    let changed = source.clone().with(path, &crlf);
    let original = render(&source);
    let first = render(&changed);
    let second = render(&changed);
    assert_eq!(first.index.as_bytes(), second.index.as_bytes());
    assert_eq!(first.by_type.as_bytes(), second.by_type.as_bytes());
    assert!(!first.index.contains('\r') && !first.by_type.contains('\r'));
    assert_ne!(first.index, original.index);
    let document: Value = serde_json::from_str(&first.index).unwrap();
    assert_eq!(
        document["resources"][0]["sources"][path],
        digest(crlf.as_bytes())
    );
    assert!(
        first
            .verify(
                Some(first.index.replace('\n', "\r\n").as_bytes()),
                Some(first.by_type.as_bytes())
            )
            .is_err()
    );
}

/// Reverse discovery order and reject any second original file read.
struct Once<'a> {
    /// The one public source slot.
    tree: &'a MemoryTree,
    /// Original reads, including generated output bytes.
    reads: RefCell<BTreeSet<String>>,
}

impl SourceTree for Once<'_> {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        let mut entries = self.tree.list(directory)?;
        entries.reverse();
        Ok(entries)
    }

    fn read(&self, path: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        assert!(
            self.reads.borrow_mut().insert(path.to_owned()),
            "second original read: {path}"
        );
        self.tree.read(path, max_bytes)
    }
}

#[test]
fn catalog_index_reuses_checked_snapshot_and_excludes_generated_inputs() {
    let source = MemoryTree::valid();
    let rendered = render(&source);
    let source = source
        .with(INDEX_PATH, &rendered.index)
        .with(BY_TYPE_PATH, &rendered.by_type);
    let once = Once {
        tree: &source,
        reads: RefCell::default(),
    };
    let checked = render(&once);
    assert_eq!(checked.index.as_bytes(), rendered.index.as_bytes());
    assert_eq!(checked.by_type.as_bytes(), rendered.by_type.as_bytes());
    checked.verify_committed().unwrap();
    let reads = once.reads.borrow();
    assert_eq!(reads.len(), 10);
    for path in [INDEX_PATH, BY_TYPE_PATH] {
        assert!(reads.contains(path));
        assert!(!checked.index.contains(path));
    }
}

#[test]
fn catalog_index_reuses_c37_version_fallback_and_common_defaults_pin() {
    let path = "settings/defaults.toml";
    let defaults = "schema = \"maestro-preferences/1\"\nlanguage = \"en\"\n";
    let source = MemoryTree::valid().with(path, defaults);
    let source = source.clone().with(
        "skills/common/SKILL.md",
        &source
            .text("skills/valid-skill/SKILL.md")
            .replace("name: valid-skill", "name: common"),
    );
    let rendered = render(&source);
    let document: Value = serde_json::from_str(&rendered.index).unwrap();
    let resources = document["resources"].as_array().unwrap();
    for row in resources {
        let id = row["id"].as_str().unwrap();
        if id == "package:common" {
            assert_eq!(row["sources"][path], digest(defaults.as_bytes()));
            assert_eq!(row["version"], "1.2.3");
        } else {
            assert!(row["sources"].get(path).is_none(), "{id}");
        }
    }
    assert!(
        resources[0]["version"].is_null(),
        "unversioned member must not invent a version"
    );
    assert_eq!(resources[3]["version"], "1.2.3", "area version fallback");
    let changed = source.edit(
        "core/agents/valid.maestro.toml",
        "maturity =",
        "version = \"9.8.7\"\nmaturity =",
    );
    let changed: Value = serde_json::from_str(&render(&changed).index).unwrap();
    assert_eq!(
        changed["resources"][0]["version"], "9.8.7",
        "declared metadata version"
    );
}

#[test]
fn catalog_index_pins_registered_inert_assets() {
    let source = MemoryTree::owned()
        .with(
            "glossaries/evidence/entry.toml",
            &format!(
                "[metadata]{}",
                package_source("package", "core")
                    .split_once("[metadata]")
                    .unwrap()
                    .1
            ),
        )
        .with(
            "glossaries/evidence/assets/example.json",
            "{\"synthetic\":true}",
        );
    let mut registry = builtin().unwrap();
    let mut descriptor = folder(&["assets/example.json"]);
    descriptor.directory = "glossaries".to_owned();
    descriptor.layout = Layout::Folder {
        file: "entry.toml".to_owned(),
        data: vec!["assets/example.json".to_owned()],
    };
    registry.register(descriptor).unwrap();
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let rendered = generate(
        &source,
        &registry,
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
    )
    .unwrap();
    let document: Value = serde_json::from_str(&rendered.index).unwrap();
    let resource = document["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "glossary:common/evidence")
        .unwrap();
    assert_eq!(
        resource["sources"]["glossaries/evidence/assets/example.json"],
        digest(b"{\"synthetic\":true}")
    );
    assert!(
        rendered.by_type.contains("## glossary\n"),
        "registered kinds drive grouping"
    );
}
