//! References across resources: dangling names and tools, dependency
//! cycles, closure maturity and setting classes.

use super::support::{MemoryTree, assert_refused};

/// The valid agent sidecar's path.
const SIDECAR: &str = "agents/base/valid.maestro.toml";
/// The valid skill's path.
const SKILL: &str = "skills/valid-skill/SKILL.md";
/// The valid preset's path.
const PRESET: &str = "presets/knowledge-client.toml";
/// The valid settings classes' path.
const CLASSES: &str = "settings/classes.toml";

/// The valid catalog with `from` replaced by `to` in `path`.
fn edited(path: &str, from: &str, to: &str) -> MemoryTree {
    MemoryTree::valid().edit(path, from, to)
}

#[test]
fn dangling_references_and_tools_are_refused() {
    assert_refused(vec![
        (
            "absent resource",
            edited(SIDECAR, "skill:valid-skill", "skill:absent"),
            "agents/base/valid.maestro.toml: requires: names skill:absent, which does not exist",
        ),
        (
            "unknown kind",
            edited(SIDECAR, "skill:valid-skill", "workflow:ctm-question"),
            "agents/base/valid.maestro.toml: requires: names workflow:ctm-question, whose kind is \
                not registered",
        ),
        (
            "not a reference",
            edited(SIDECAR, "skill:valid-skill", "valid-skill"),
            "agents/base/valid.maestro.toml: requires: \"valid-skill\" is not a kind:name \
                reference",
        ),
        (
            "kind not admitted",
            edited(
                "mcp/maestro.toml",
                "workflows = [\"ctm-question\"]",
                "workflows = [\"ctm-question\"]\nrequires = [\"skill:valid-skill\"]",
            ),
            "mcp/maestro.toml: metadata.requires: kind mcp may not require skill:valid-skill",
        ),
        (
            "unlisted tool",
            edited(
                "agents/base/valid.agent.md",
                "maestro/knowledge_search",
                "maestro/absent",
            ),
            "agents/base/valid.agent.md: tools: names maestro/absent, which mcp:maestro does not \
                list",
        ),
        (
            "absent server",
            edited(
                "agents/base/valid.agent.md",
                "maestro/knowledge_search",
                "other/knowledge_search",
            ),
            "agents/base/valid.agent.md: tools: names other/knowledge_search, and mcp:other does \
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
            "  maestro.requires: skill:valid-skill\n  maestro.workflows",
        )
        .edit(
            SKILL,
            "  maestro.workflows",
            "  maestro.requires: skill:loop-skill\n  maestro.workflows",
        );
    assert_refused(vec![(
        "two skills",
        looped,
        "skills/loop-skill/SKILL.md: metadata.maestro.requires: dependency cycle among \
            skill:loop-skill, skill:valid-skill",
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
            "presets/knowledge-client.toml: metadata.requires: closure member skill:valid-skill is \
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

#[test]
fn every_setting_has_exactly_one_known_class() {
    assert_refused(vec![
        (
            "unknown key",
            edited(CLASSES, "additive = []", "additive = [\"colour\"]"),
            "settings/classes.toml: classes.additive: unknown setting \"colour\"",
        ),
        (
            "unclassified key",
            edited(CLASSES, "\"model_profile\", ", ""),
            "settings/classes.toml: classes: setting \"model_profile\" has no class",
        ),
        (
            "doubly classified key",
            edited(CLASSES, "additive = []", "additive = [\"model_profile\"]"),
            "settings/classes.toml: classes: setting \"model_profile\" has two classes",
        ),
        (
            "preset key",
            edited(PRESET, "tone = \"normal\"", "colour = \"blue\""),
            "presets/knowledge-client.toml: settings.colour: unknown setting",
        ),
    ]);
}
