//! References across resources: dangling names and tools, dependency
//! cycles, closure maturity and preset setting keys.

use super::{
    area_packages::package_source,
    support::{MemoryTree, assert_refused, check_under},
};
use crate::limits::Limits;

/// The valid agent sidecar's path.
const SIDECAR: &str = "core/agents/valid.maestro.toml";
/// The valid skill's path.
const SKILL: &str = "skills/valid-skill/SKILL.md";
/// The valid preset's path.
const PRESET: &str = "presets/knowledge-client.toml";

/// The valid catalog with `from` replaced by `to` in `path`.
fn edited(path: &str, from: &str, to: &str) -> MemoryTree {
    MemoryTree::valid().edit(path, from, to)
}

#[test]
fn dangling_references_and_tools_are_refused() {
    assert_refused(vec![
        (
            "absent resource",
            edited(SIDECAR, "skill:common/valid-skill", "skill:common/absent"),
            "core/agents/valid.maestro.toml: requires: names skill:common/absent, which does \
            not exist",
        ),
        (
            "unknown kind",
            edited(
                SIDECAR,
                "skill:common/valid-skill",
                "workflow:core/ctm-question",
            ),
            "core/agents/valid.maestro.toml: requires: names workflow:core/ctm-question, \
            whose kind is \
                not registered",
        ),
        (
            "not a reference",
            edited(SIDECAR, "skill:common/valid-skill", "valid-skill"),
            "core/agents/valid.maestro.toml: requires: \"valid-skill\" is not a qualified \
            kind:namespace/local-name reference",
        ),
        (
            "kind not admitted",
            edited(
                SIDECAR,
                "skill:common/valid-skill",
                "preset:knowledge-client",
            ),
            "core/agents/valid.maestro.toml: requires: kind agent may not require \
            preset:knowledge-client",
        ),
        (
            "unlisted tool",
            edited("core/agents/valid.agent.md", "view", "maestro/absent"),
            "core/agents/valid.agent.md: tools: names maestro/absent, and mcp:maestro does not \
                exist",
        ),
        (
            "absent server",
            edited(
                "core/agents/valid.agent.md",
                "view",
                "other/knowledge_search",
            ),
            "core/agents/valid.agent.md: tools: names other/knowledge_search, and mcp:other does \
                not exist",
        ),
    ]);
}

#[test]
fn dependency_cycles_are_refused() {
    let valid = MemoryTree::valid();
    let looped = valid
        .clone()
        .with("skills/loop-skill/SKILL.md", &valid.text(SKILL))
        .edit(
            "skills/loop-skill/SKILL.md",
            "name: valid-skill",
            "name: loop-skill",
        )
        .edit(
            "skills/loop-skill/SKILL.md",
            "  maestro.workflows",
            "  maestro.requires: skill:common/valid-skill\n  maestro.workflows",
        )
        .edit(
            SKILL,
            "  maestro.workflows",
            "  maestro.requires: skill:common/loop-skill\n  maestro.workflows",
        );
    assert_refused(vec![(
        "two skills",
        looped,
        "skills/loop-skill/SKILL.md: metadata.maestro.requires: dependency cycle among \
            skill:common/loop-skill, skill:common/valid-skill",
    )]);
}

#[test]
fn closure_members_must_be_reviewed_beside_a_reviewed_neighbour() {
    let stage = |maturity: &str| {
        edited(
            SKILL,
            "maestro.maturity: reviewed",
            &format!("maestro.maturity: {maturity}"),
        )
    };
    let message = |maturity: &str| {
        format!(
            "presets/knowledge-client.toml: metadata.requires: closure member \
            skill:common/valid-skill is \
                {maturity}; a closure admits only reviewed members"
        )
    };
    let (placeholder, authored, retired) = (
        message("placeholder"),
        message("authored"),
        message("retired"),
    );
    assert_refused(vec![
        ("placeholder member", stage("placeholder"), &placeholder),
        ("authored member", stage("authored"), &authored),
        ("retired member", stage("retired"), &retired),
    ]);
}

/// A reviewed area whose explicit dependencies are the only selection inputs.
pub(super) fn area(
    tree: MemoryTree,
    kind: &str,
    name: &str,
    path: &str,
    requires: &[&str],
) -> MemoryTree {
    let references = requires
        .iter()
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>();
    tree.with(
        path,
        &package_source(kind, name).replace(
            "requires = []",
            &format!("requires = [{}]", references.join(", ")),
        ),
    )
}

#[test]
fn core_optional_language_refuses_beside_global_language() {
    let tree = area(
        MemoryTree::default(),
        "language",
        "rust",
        "languages/rust/package.toml",
        &[],
    );
    let tree = area(
        tree,
        "package",
        "core",
        "core/package.toml",
        &["language:rust"],
    );
    let global = area(
        tree.clone(),
        "package",
        "common",
        "package.toml",
        &["language:rust"],
    );
    assert!(check_under(&global, &Limits::PRODUCTION).is_ok());
    let result = check_under(&tree, &Limits::PRODUCTION);
    assert!(result.is_err(), "optional language is not global");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("layer may not require language:rust")
    );
}

#[test]
fn global_languages_follow_transitive_area_requirements() {
    let tree = area(
        MemoryTree::default(),
        "standard",
        "security",
        "standards/security/package.toml",
        &["language:rust"],
    );
    let tree = area(
        tree,
        "language",
        "rust",
        "languages/rust/package.toml",
        &["language:python"],
    );
    let tree = area(
        tree,
        "language",
        "python",
        "languages/python/package.toml",
        &[],
    );
    let skill = MemoryTree::valid().text(SKILL);
    let tree = tree.with("languages/python/skills/valid-skill/SKILL.md", &skill);
    let tree = area(
        tree,
        "package",
        "core",
        "core/package.toml",
        &["skill:python/valid-skill"],
    );
    assert!(
        check_under(&tree, &Limits::PRODUCTION).is_ok(),
        "transitive Global language resources must be eligible"
    );
    let optional = tree.edit(
        "languages/rust/package.toml",
        "requires = [\"language:python\"]",
        "requires = []",
    );
    assert!(
        check_under(&optional, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string()
            .contains("layer may not require skill:python/valid-skill")
    );
}

/// A selected team package and a surviving same-basename common resource.
fn removable_tree() -> MemoryTree {
    let skill = MemoryTree::valid().text(SKILL);
    let tree = MemoryTree::valid()
        .with(
            "capabilities/practice/review/skills/valid-skill/SKILL.md",
            &skill,
        )
        .edit(
            PRESET,
            "agent:core/valid",
            "agent:core/valid\", \"package:other\", \"package:review\", \"skill:review/valid-skill",
        );
    let tree = [
        ("package", "common", "package.toml"),
        ("package", "core", "core/package.toml"),
        ("standard", "security", "standards/security/package.toml"),
        (
            "package",
            "other",
            "capabilities/practice/other/package.toml",
        ),
    ]
    .into_iter()
    .fold(tree, |tree, (kind, name, path)| {
        area(tree, kind, name, path, &[])
    });
    area(
        tree,
        "package",
        "review",
        "capabilities/practice/review/package.toml",
        &["skill:review/valid-skill"],
    )
}

#[test]
fn removed_package_dangling_reference_refuses() {
    let tree = removable_tree();
    assert!(check_under(&tree, &Limits::PRODUCTION).is_ok());
    let tree = tree
        .without("capabilities/practice/review/package.toml")
        .without("capabilities/practice/review/skills/valid-skill/SKILL.md");
    let result = check_under(&tree, &Limits::PRODUCTION);
    assert!(
        result.is_err(),
        "removal must refuse every surviving missing edge"
    );
    let result = result.unwrap_err();
    let missing: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.message.contains("does not exist"))
        .collect();
    assert_eq!(missing.len(), 2, "every surviving edge: {result}");
    for id in ["package:review", "skill:review/valid-skill"] {
        assert!(
            missing
                .iter()
                .any(|diagnostic| diagnostic.path == PRESET && diagnostic.message.contains(id))
        );
    }
}

#[test]
fn package_removal_keeps_core_bytes() {
    let tree = removable_tree();
    let before = check_under(&tree, &Limits::PRODUCTION).unwrap();
    let removed = tree
        .clone()
        .without("capabilities/practice/review/package.toml")
        .without("capabilities/practice/review/skills/valid-skill/SKILL.md")
        .edit(
            PRESET,
            ", \"package:review\", \"skill:review/valid-skill\"",
            "",
        );
    let after = check_under(&removed, &Limits::PRODUCTION).unwrap();
    for path in [
        "package.toml",
        "core/package.toml",
        "standards/security/package.toml",
        "capabilities/practice/other/package.toml",
        SIDECAR,
        "core/agents/valid.agent.md",
        SKILL,
        "core/instructions/valid.instructions.md",
        "core/instructions/valid.maestro.toml",
    ] {
        assert_eq!(removed.text(path), tree.text(path));
    }
    let surviving: Vec<_> = before
        .resources
        .into_iter()
        .filter(|resource| {
            resource.id.namespace.as_deref() != Some("review")
                && resource.id.name != "review"
                && resource.id.kind != "preset"
        })
        .collect();
    assert_eq!(
        after
            .resources
            .into_iter()
            .filter(|resource| resource.id.kind != "preset")
            .collect::<Vec<_>>(),
        surviving
    );
}

#[test]
fn presets_include_required_roots_without_injecting_available_personas() {
    for (kind, name, path) in [
        ("package", "common", "package.toml"),
        ("package", "core", "core/package.toml"),
        ("standard", "security", "standards/security/package.toml"),
    ] {
        let tree = area(
            MemoryTree::valid(),
            kind,
            name,
            path,
            &["skill:common/valid-skill"],
        )
        .edit(PRESET, "requires = [\"agent:core/valid\"]", "requires = []");
        let checked = check_under(&tree, &Limits::PRODUCTION).unwrap();
        assert!(
            checked
                .resources
                .iter()
                .find(|resource| resource.id.kind == "preset")
                .unwrap()
                .metadata
                .requires
                .is_empty()
        );
        let authored = tree.clone().edit(
            SKILL,
            "maestro.maturity: reviewed",
            "maestro.maturity: authored",
        );
        let result = check_under(&authored, &Limits::PRODUCTION).unwrap_err();
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.path == PRESET
                    && diagnostic
                        .message
                        .contains("closure member skill:common/valid-skill is authored")),
            "{result}"
        );
        let available = tree.edit(
            SIDECAR,
            "maturity = \"reviewed\"",
            "maturity = \"authored\"",
        );
        assert!(
            check_under(&available, &Limits::PRODUCTION).is_ok(),
            "workflow labels and availability must select nothing"
        );
    }
}

#[test]
fn preset_unknown_setting_keys_are_refused() {
    assert_refused(vec![(
        "preset key",
        edited(PRESET, "tone = \"normal\"", "colour = \"blue\""),
        "presets/knowledge-client.toml: settings.colour: unknown setting",
    )]);
}
