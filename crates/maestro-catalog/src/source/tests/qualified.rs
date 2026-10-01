//! C32 qualified identities and the single source schema cutover.

use super::{
    area_packages::package_source,
    registry::glossary,
    support::{MemoryTree, check_by},
};
use crate::{
    limits::Limits,
    source::{Layout, Registry, ResourceId, Scope, builtin},
};

/// A scoped data-only kind, with two placements sharing one namespace.
fn registry() -> Registry {
    let mut registry = builtin().unwrap();
    let mut descriptor = glossary();
    descriptor.scopes = vec![
        Scope::Common,
        Scope::Core,
        Scope::Team,
        Scope::Language,
        Scope::Standard,
    ];
    descriptor.layout = Layout::Files {
        suffix: ".toml".to_owned(),
        folders: vec!["one".to_owned(), "two".to_owned()],
    };
    registry.register(descriptor).unwrap();
    registry
}

/// A checked data-only resource.
fn entry() -> String {
    format!(
        "term = \"evidence\"\n[metadata]{}",
        package_source("package", "core")
            .split_once("[metadata]")
            .unwrap()
            .1
    )
}

#[test]
fn same_stem_different_kind_accepts() {
    let tree = MemoryTree::default()
        .with("core/glossaries/one/evidence.toml", &entry())
        .with(
            "core/skills/evidence/SKILL.md",
            &MemoryTree::valid()
                .text("skills/valid-skill/SKILL.md")
                .replace("valid-skill", "evidence"),
        );
    let checked = check_by(&tree, &registry(), &Limits::PRODUCTION).unwrap();
    let ids: Vec<_> = checked
        .resources
        .iter()
        .map(|resource| resource.id.to_string())
        .collect();
    assert_eq!(ids, ["glossary:core/evidence", "skill:core/evidence"]);
}

#[test]
fn duplicate_kind_namespace_name_refuses() {
    let tree = MemoryTree::default()
        .with("core/glossaries/one/evidence.toml", &entry())
        .with("core/glossaries/two/evidence.toml", &entry());
    let refusal = check_by(&tree, &registry(), &Limits::PRODUCTION).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("duplicate ID glossary:core/evidence"),
        "{refusal}"
    );
}

#[test]
fn duplicate_area_namespace_refuses() {
    for (kind, path) in [
        ("package", "capabilities/practice/core/package.toml"),
        ("language", "languages/core/package.toml"),
        ("standard", "standards/core/package.toml"),
    ] {
        let tree = MemoryTree::default()
            .with("core/package.toml", &package_source("package", "core"))
            .with(path, &package_source(kind, "core"));
        let result = check_by(&tree, &builtin().unwrap(), &Limits::PRODUCTION);
        assert!(result.is_err(), "duplicate namespace must refuse: {path}");
        let refusal = result.unwrap_err();
        assert!(
            refusal
                .to_string()
                .contains("duplicate area namespace core"),
            "{refusal}"
        );
    }
}

#[test]
fn old_or_mixed_layout_refuses() {
    for path in [
        "capability.toml",
        "capabilities/practice/review/capability.toml",
        "agents/base/old.agent.md",
        "model-cards/old.toml",
        "mcp/server.toml",
    ] {
        let tree = MemoryTree::default()
            .with("core/package.toml", &package_source("package", "core"))
            .with(path, "old content");
        let refusal = check_by(&tree, &builtin().unwrap(), &Limits::PRODUCTION).unwrap_err();
        assert!(
            refusal.diagnostics.iter().any(|diagnostic| {
                diagnostic.path == path && diagnostic.message.contains("migrate")
            }),
            "{refusal}"
        );
    }
    let old = package_source("package", "core").replace("maestro-source/2", "maestro-source/1");
    let result = check_by(
        &MemoryTree::default().with("core/package.toml", &old),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
    );
    assert!(result.is_err(), "old envelope must refuse");
    assert!(result.unwrap_err().to_string().contains("migrate"));
}

#[test]
fn old_and_malformed_ids_refuse_without_rebinding() {
    for id in [
        "capability:review",
        "capability:core/review",
        "Skill:core/name",
        "skill:valid-skill",
        "skill:core/",
        "skill:/name",
        "skill:core/a/b",
        "skill:Core/name",
        "package:core/name",
    ] {
        let source = package_source("package", "core")
            .replace("requires = []", &format!("requires = [\"{id}\"]"));
        let refusal = check_by(
            &MemoryTree::default().with("core/package.toml", &source),
            &builtin().unwrap(),
            &Limits::PRODUCTION,
        )
        .unwrap_err();
        assert!(
            refusal
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.key == "metadata.requires"
                    && diagnostic.message.contains("reference")),
            "{id}: {refusal}"
        );
    }
}

#[test]
fn root_and_namespace_ids_roundtrip() {
    let package = package_source("package", "core");
    let metadata = package.split_once("[metadata]").unwrap().1.replace(
        "requires = []",
        "requires = [\"package:common\", \"package:core\", \"package:review\", \
         \"language:rust\", \"standard:quality\", \"glossary:common/evidence\"]",
    );
    let preset = format!("name = \"valid\"\ndescription = \"Synthetic\"\n[metadata]{metadata}");
    let tree = MemoryTree::default()
        .with("package.toml", &package_source("package", "common"))
        .with("core/package.toml", &package_source("package", "core"))
        .with(
            "languages/rust/package.toml",
            &package_source("language", "rust"),
        )
        .with(
            "standards/quality/package.toml",
            &package_source("standard", "quality"),
        )
        .with(
            "capabilities/practice/review/package.toml",
            &package_source("package", "review"),
        )
        .with(
            "capabilities/practice/review/glossaries/one/evidence.toml",
            &entry(),
        )
        .with("glossaries/one/evidence.toml", &entry())
        .with("languages/rust/glossaries/one/evidence.toml", &entry())
        .with("standards/quality/glossaries/one/evidence.toml", &entry())
        .with("core/glossaries/one/evidence.toml", &entry())
        .with("presets/valid.toml", &preset);
    let checked = check_by(&tree, &registry(), &Limits::PRODUCTION).unwrap();
    let ids: Vec<_> = checked
        .resources
        .iter()
        .map(|resource| resource.id.to_string())
        .collect();
    assert_eq!(
        ids,
        [
            "glossary:common/evidence",
            "glossary:core/evidence",
            "glossary:quality/evidence",
            "glossary:review/evidence",
            "glossary:rust/evidence",
            "language:rust",
            "package:common",
            "package:core",
            "package:review",
            "preset:valid",
            "standard:quality"
        ]
    );
}

#[test]
fn duplicate_namespace_without_area_descriptor_refuses() {
    let tree = MemoryTree::default()
        .with("core/glossaries/one/evidence.toml", &entry())
        .with(
            "capabilities/practice/core/glossaries/one/other.toml",
            &entry(),
        );
    let result = check_by(&tree, &registry(), &Limits::PRODUCTION);
    assert!(
        result.is_err(),
        "distinct resource IDs cannot mask duplicate namespaces"
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("duplicate area namespace core")
    );
}

#[test]
fn identity_serialization_is_a_typed_golden_vector() {
    let tree = MemoryTree::default().with("core/glossaries/one/evidence.toml", &entry());
    let id = &check_by(&tree, &registry(), &Limits::PRODUCTION)
        .unwrap()
        .resources[0]
        .id;
    let bytes = serde_json::to_vec(id).unwrap();
    assert_eq!(
        bytes,
        br#"{"kind":"glossary","namespace":"core","name":"evidence"}"#
    );
    let decoded: ResourceId = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, *id);
    let root = ResourceId {
        kind: "package".to_owned(),
        namespace: None,
        name: "core".to_owned(),
    };
    assert_eq!(
        serde_json::to_vec(&root).unwrap(),
        br#"{"kind":"package","namespace":null,"name":"core"}"#
    );
}

#[test]
fn qualified_segments_keep_the_64_character_boundary() {
    let name = "a".repeat(64);
    let path = format!("capabilities/practice/{name}/glossaries/one/{name}.toml");
    let tree = MemoryTree::default().with(&path, &entry());
    let checked = check_by(&tree, &registry(), &Limits::PRODUCTION).unwrap();
    assert_eq!(
        checked.resources[0].id.to_string(),
        format!("glossary:{name}/{name}")
    );
    for id in [
        format!("glossary:{name}a/{name}"),
        format!("glossary:{name}/{name}a"),
    ] {
        let text = package_source("package", "core")
            .replace("requires = []", &format!("requires = [\"{id}\"]"));
        let result = check_by(
            &MemoryTree::default().with("core/package.toml", &text),
            &registry(),
            &Limits::PRODUCTION,
        );
        assert!(result.unwrap_err().to_string().contains("not a qualified"));
    }
}
