//! References across resources: dangling names and tools, dependency
//! cycles, closure maturity and preset setting keys.

use super::support::{MemoryTree, assert_refused};

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

#[test]
fn preset_unknown_setting_keys_are_refused() {
    assert_refused(vec![(
        "preset key",
        edited(PRESET, "tone = \"normal\"", "colour = \"blue\""),
        "presets/knowledge-client.toml: settings.colour: unknown setting",
    )]);
}
