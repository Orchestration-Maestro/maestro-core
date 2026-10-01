//! The catalog's layout: agent and sidecar pairing, duplicate IDs, entries
//! that are not resources, unsupported kinds and links.

use super::support::{MemoryTree, assert_refused};
use std::collections::BTreeSet;

/// The valid agent profile's path.
const AGENT: &str = "agents/base/valid.agent.md";
/// The valid agent sidecar's path.
const SIDECAR: &str = "agents/base/valid.maestro.toml";

#[test]
fn agent_name_must_equal_its_stem_and_pair_one_sidecar() {
    let valid = MemoryTree::valid();
    assert_refused(vec![
        (
            "stem and name differ",
            valid.clone().edit(AGENT, "name: valid", "name: other"),
            "agents/base/valid.agent.md: name: must equal the file stem \"valid\", not \"other\"",
        ),
        (
            "no sidecar",
            valid.clone().without(SIDECAR),
            "agents/base/valid.agent.md: no valid.maestro.toml sidecar beside it",
        ),
        (
            "ambiguous sidecar",
            valid
                .clone()
                .with("agents/base/other.maestro.toml", &valid.text(SIDECAR))
                .with("agents/base/renamed.agent.md", &valid.text(AGENT))
                .edit("agents/base/renamed.agent.md", "name: valid", "name: other"),
            "agents/base/other.maestro.toml: no other.agent.md beside it",
        ),
        (
            "instructions without sidecar",
            valid.clone().without("instructions/valid.maestro.toml"),
            "instructions/valid.instructions.md: no valid.maestro.toml sidecar beside it",
        ),
    ]);
}

#[test]
fn duplicate_ids_across_directories_are_refused() {
    let valid = MemoryTree::valid();
    assert_refused(vec![(
        "same agent twice",
        valid
            .clone()
            .with(
                "agents/capabilities/review/valid.agent.md",
                &valid.text(AGENT),
            )
            .with(
                "agents/capabilities/review/valid.maestro.toml",
                &valid.text(SIDECAR),
            ),
        "agents/capabilities/review/valid.agent.md: duplicate ID agent:valid, also \
            agents/base/valid.agent.md",
    )]);
}

#[test]
fn unsupported_kinds_stray_entries_and_links_are_refused() {
    let valid = MemoryTree::valid();
    assert_refused(vec![
        (
            "workflow graph",
            valid
                .clone()
                .with("workflows/ctm-question/workflow.md", "---\n---\n"),
            "workflows: no kind registered for this directory",
        ),
        (
            "policy",
            valid.clone().with(
                "policies/base.cedar",
                "permit(principal, action, resource);\n",
            ),
            "policies: no kind registered for this directory",
        ),
        (
            "stray top-level file",
            valid.clone().with("notes.txt", "notes\n"),
            "notes.txt: not a catalog resource or a known non-resource entry",
        ),
        (
            "stray resource file",
            valid.clone().with("agents/base/notes.txt", "notes\n"),
            "agents/base/notes.txt: not a resource file of this directory",
        ),
        (
            "stray skill entry",
            valid
                .clone()
                .with("skills/valid-skill/run.sh", "#!/bin/sh\n"),
            "skills/valid-skill/run.sh: not a resource file of this directory",
        ),
        (
            "link",
            valid.clone().with_link("agents/base/linked.agent.md"),
            "agents/base/linked.agent.md: links and special files are not read",
        ),
        (
            "not UTF-8",
            valid.with_bytes(SIDECAR, b"owner = \"\xff\"\n"),
            "agents/base/valid.maestro.toml: not UTF-8",
        ),
    ]);
}

#[test]
fn v4_area_placement_accepts() {
    use super::area_support::{discover, registry};
    use crate::limits::Limits;
    let paths = [
        "glossaries/common.toml",
        "core/glossaries/framework.toml",
        "capabilities/practice/review/glossaries/team.toml",
        "languages/rust/glossaries/language.toml",
        "standards/quality/glossaries/standard.toml",
    ];
    let tree = paths
        .iter()
        .fold(MemoryTree::default(), |tree, path| tree.with(path, "data"));
    let found = discover(&tree, &registry(), &Limits::PRODUCTION);
    assert!(found.diagnostics.is_empty(), "{:#?}", found.diagnostics);
    assert_eq!(
        found
            .units
            .iter()
            .map(|unit| unit.path.as_str())
            .collect::<BTreeSet<_>>(),
        paths.into_iter().collect()
    );
}

#[test]
fn nested_or_unknown_area_refuses() {
    use super::area_support::{refuses, registry};
    for path in [
        "core/languages/rust/glossaries/bad.toml",
        "languages/rust/core/glossaries/bad.toml",
        "capabilities/practice/team/capabilities/security/inner/glossaries/bad.toml",
        "unknown/glossaries/bad.toml",
        "extensions/bad/extension.toml",
        "core/extensions/bad/extension.toml",
        "knowledge/collections/bad/collection.json",
        "agents/bad.agent.md",
        "instructions/bad.md",
        "policies/bad.cedar",
    ] {
        refuses(
            &MemoryTree::default().with(path, "data"),
            &registry(),
            path,
            "not a registered v4 placement",
        );
    }
}

#[test]
fn scoped_support_roots_are_checked_not_skipped() {
    use super::area_support::{discover, refuses, scoped};
    use crate::{limits::Limits, source::Registry};
    let mut registry = Registry::default();
    registry
        .register(scoped("docs/catalog", &["root"]))
        .unwrap();
    let tree = MemoryTree::default().with("docs/catalog/guide.toml", "data");
    let found = discover(&tree, &registry, &Limits::PRODUCTION);
    assert!(found.diagnostics.is_empty(), "{:#?}", found.diagnostics);
    assert_eq!(found.units.len(), 1);
    refuses(
        &tree.with("docs/unchecked.md", "data"),
        &registry,
        "docs/unchecked.md",
        "not a registered v4 placement",
    );
}

#[test]
fn scoped_assets_are_exact_and_owner_local() {
    use super::area_support::{discover, folder, refuses};
    use crate::{limits::Limits, source::Registry};
    let mut registry = Registry::default();
    registry.register(folder(&["assets/example.json"])).unwrap();
    let tree = MemoryTree::default()
        .with("skills/review/SKILL.md", "data")
        .with("skills/review/assets/example.json", "{}");
    let found = discover(&tree, &registry, &Limits::PRODUCTION);
    assert!(found.diagnostics.is_empty(), "{:#?}", found.diagnostics);
    assert_eq!(found.units[0].data, ["skills/review/assets/example.json"]);
    refuses(
        &tree.clone().with("skills/review/assets/other.json", "{}"),
        &registry,
        "skills/review/assets/other.json",
        "not a registered v4 placement",
    );
    refuses(
        &tree.with_link("skills/review/assets/link.json"),
        &registry,
        "skills/review/assets/link.json",
        "links and special files",
    );
}

#[test]
fn functional_naming_exception_is_exact() {
    use super::area_support::{discover, scoped};
    use crate::{
        limits::Limits,
        source::{Layout, Registry},
    };
    for (directory, file, scope) in [
        ("hosts/pi", "config.toml", "core"),
        ("hosts/claude-code", "config.toml", "core"),
        ("hosts/copilot", "config.toml", "core"),
        ("bootstrap/base/files", "AGENTS.md", "common"),
        ("bootstrap/base/files", "CLAUDE.md", "common"),
        (
            "bootstrap/base/files/.github",
            "copilot-instructions.md",
            "common",
        ),
        ("skills/review", "SKILL.md", "common"),
    ] {
        for (prefix, accepted) in [("", true), ("nested/", false)] {
            let directory = format!("{prefix}{directory}");
            let mut descriptor = scoped(&directory, &[scope]);
            descriptor.layout = Layout::Single {
                file: file.to_owned(),
                name: "valid".to_owned(),
            };
            let mut registry = Registry::default();
            registry.register(descriptor).unwrap();
            let path = format!(
                "{}{directory}/{file}",
                if scope == "core" { "core/" } else { "" }
            );
            let found = discover(
                &MemoryTree::default().with(&path, "data"),
                &registry,
                &Limits::PRODUCTION,
            );
            assert_eq!(
                found.diagnostics.is_empty(),
                accepted,
                "{path}: {:#?}",
                found.diagnostics
            );
            if accepted {
                assert_eq!(found.units.len(), 1, "{path}");
            }
        }
    }
}

#[test]
fn registered_product_names_and_tokens_refuse_without_self_exemptions() {
    use super::area_support::{discover, registry};
    use crate::limits::Limits;
    for name in [
        "pi",
        "claude-code",
        "copilot",
        "ladybug",
        "qdrant",
        "gerrit",
        "ladybug-store",
        "store_qdrant",
        "store.gerrit",
        "LadyBug",
        "helper-claude-code",
    ] {
        let path = format!("glossaries/{name}.toml");
        let found = discover(
            &MemoryTree::default().with(&path, "data"),
            &registry(),
            &Limits::PRODUCTION,
        );
        assert!(
            found
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.path == path
                    && diagnostic.message.contains("not a registered product")),
            "{path}: {:#?}",
            found.diagnostics
        );
    }
    let found = discover(
        &MemoryTree::default().with("glossaries/ladybugs.toml", "data"),
        &registry(),
        &Limits::PRODUCTION,
    );
    assert!(found.diagnostics.is_empty(), "{:#?}", found.diagnostics);
}

#[test]
fn scoped_sidecar_pairs_once_and_missing_sidecar_refuses() {
    use super::area_support::{discover, refuses, scoped};
    use crate::{
        limits::Limits,
        source::{MetadataPlace, Registry},
    };
    let mut descriptor = scoped("glossaries", &["common"]);
    descriptor.metadata = MetadataPlace::Sidecar {
        suffix: ".maestro.toml".to_owned(),
    };
    let mut registry = Registry::default();
    registry.register(descriptor).unwrap();
    let tree = MemoryTree::default()
        .with("glossaries/valid.toml", "data")
        .with("glossaries/valid.maestro.toml", "metadata");
    let found = discover(&tree, &registry, &Limits::PRODUCTION);
    assert!(found.diagnostics.is_empty(), "{:#?}", found.diagnostics);
    assert_eq!(found.units.len(), 1);
    assert_eq!(
        found.units[0].sidecar.as_deref(),
        Some("glossaries/valid.maestro.toml")
    );
    refuses(
        &tree.without("glossaries/valid.maestro.toml"),
        &registry,
        "glossaries/valid.toml",
        "sidecar beside it",
    );
}

#[test]
fn scoped_link_primary_refuses_without_another_guard() {
    use super::area_support::{discover, registry};
    use crate::limits::Limits;
    let found = discover(
        &MemoryTree::default().with_link("glossaries/valid.toml"),
        &registry(),
        &Limits::PRODUCTION,
    );
    assert_eq!(found.diagnostics.len(), 1, "{:#?}", found.diagnostics);
    assert!(
        found.diagnostics[0]
            .message
            .contains("links and special files")
    );
}

#[test]
fn scoped_inventory_requires_its_exact_asset() {
    use super::area_support::{discover, folder};
    use crate::{limits::Limits, source::Registry};
    let mut registry = Registry::default();
    registry.register(folder(&["assets/example.json"])).unwrap();
    let found = discover(
        &MemoryTree::default().with("skills/review/SKILL.md", "data"),
        &registry,
        &Limits::PRODUCTION,
    );
    assert_eq!(found.diagnostics.len(), 1);
    assert_eq!(found.diagnostics[0].message, "missing inventoried asset");
}

#[test]
fn scoped_local_names_are_functional_without_path_masking() {
    use super::area_support::{discover, scoped};
    use crate::{
        limits::Limits,
        source::{Layout, Registry},
    };
    let mut descriptor = scoped("glossaries", &["common"]);
    descriptor.layout = Layout::Single {
        file: "config.toml".to_owned(),
        name: "qdrant".to_owned(),
    };
    let mut registry = Registry::default();
    registry.register(descriptor).unwrap();
    let found = discover(
        &MemoryTree::default().with("glossaries/config.toml", "data"),
        &registry,
        &Limits::PRODUCTION,
    );
    assert_eq!(found.diagnostics.len(), 1);
    assert!(found.diagnostics[0].message.contains("functional name"));
    let found = discover(
        &MemoryTree::default().with("glossaries/bad_name.toml", "data"),
        &super::area_support::registry(),
        &Limits::PRODUCTION,
    );
    assert_eq!(found.diagnostics.len(), 1);
    assert!(
        found.diagnostics[0]
            .message
            .contains("lower-case hyphenated")
    );
}

#[test]
fn scoped_primary_required_root_and_orphan_sidecar_refuse() {
    use super::area_support::{discover, folder, scoped};
    use crate::{
        limits::Limits,
        source::{MetadataPlace, Registry},
    };
    let mut registry = Registry::default();
    registry.register(folder(&["assets/example.json"])).unwrap();
    let found = discover(
        &MemoryTree::default().with("skills/review/assets/example.json", "{}"),
        &registry,
        &Limits::PRODUCTION,
    );
    assert_eq!(found.diagnostics.len(), 1);
    assert_eq!(found.diagnostics[0].message, "no SKILL.md");
    let mut descriptor = scoped("glossaries", &["common"]);
    descriptor.required = Some("must hold a glossary".to_owned());
    let mut registry = Registry::default();
    registry.register(descriptor).unwrap();
    let tree = MemoryTree::default().with("glossaries/not-a-resource.txt", "data");
    let found = discover(&tree, &registry, &Limits::PRODUCTION);
    assert!(
        found
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "missing: must hold a glossary")
    );
    let mut descriptor = scoped("glossaries", &["common"]);
    descriptor.metadata = MetadataPlace::Sidecar {
        suffix: ".maestro.toml".to_owned(),
    };
    let mut registry = Registry::default();
    registry.register(descriptor).unwrap();
    let found = discover(
        &MemoryTree::default().with("glossaries/orphan.maestro.toml", "data"),
        &registry,
        &Limits::PRODUCTION,
    );
    assert_eq!(found.diagnostics.len(), 1);
    assert!(
        found.diagnostics[0]
            .message
            .contains("not a registered v4 placement")
    );
}

#[test]
fn product_path_refuses_without_local_name_masking() {
    use super::area_support::{discover, scoped};
    use crate::{
        limits::Limits,
        source::{Layout, Registry},
    };
    for name in [
        "pi",
        "claude-code",
        "copilot",
        "ladybug",
        "qdrant",
        "gerrit",
        "ladybug-store",
        "store_qdrant",
        "store.gerrit",
        "LadyBug",
    ] {
        let directory = format!("glossaries/{name}");
        let path = format!("{directory}/config.toml");
        let mut descriptor = scoped(&directory, &["common"]);
        descriptor.layout = Layout::Single {
            file: "config.toml".to_owned(),
            name: "valid".to_owned(),
        };
        let mut registry = Registry::default();
        registry.register(descriptor).unwrap();
        let found = discover(
            &MemoryTree::default().with(&path, "data"),
            &registry,
            &Limits::PRODUCTION,
        );
        assert!(
            !found.diagnostics.is_empty(),
            "product path must refuse: {path}"
        );
        assert!(
            found
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.message.contains("not a registered product"))
        );
    }
}

#[test]
fn unregistered_extension_refuses_without_another_guard() {
    use super::area_support::{discover, registry};
    use crate::limits::Limits;
    let found = discover(
        &MemoryTree::default().with("core/extensions/editor/extension.toml", "data"),
        &registry(),
        &Limits::PRODUCTION,
    );
    assert_eq!(found.diagnostics.len(), 1);
    assert!(
        found.diagnostics[0]
            .message
            .contains("not a registered v4 placement")
    );
}

#[test]
fn unregistered_collection_refuses_without_another_guard() {
    use super::area_support::{discover, registry};
    use crate::limits::Limits;
    let found = discover(
        &MemoryTree::default().with("knowledge/collections/example/collection.json", "data"),
        &registry(),
        &Limits::PRODUCTION,
    );
    assert_eq!(found.diagnostics.len(), 1);
    assert!(
        found.diagnostics[0]
            .message
            .contains("not a registered v4 placement")
    );
}
