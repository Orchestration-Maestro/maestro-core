//! Each file's strict schema: duplicate and unknown keys, wrong types,
//! versions, stages, owners, rows, workflows and fixed agent sections.

use super::support::{INVALID_AGENT, MemoryTree, assert_refused};

/// The valid agent profile's path.
const AGENT: &str = "core/agents/valid.agent.md";
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
fn duplicate_keys_are_refused_in_yaml_and_toml() {
    assert_refused(vec![
        (
            "frontmatter",
            edited(AGENT, "name: valid\n", "name: valid\nname: valid\n"),
            "core/agents/valid.agent.md: invalid frontmatter: duplicate key \"name\"",
        ),
        (
            "sidecar",
            edited(SIDECAR, "maturity", "owner = \"x\"\nmaturity"),
            "core/agents/valid.maestro.toml: invalid TOML at line 3, column 1: duplicate key",
        ),
        (
            "skill metadata",
            edited(
                SKILL,
                "  maestro.maturity",
                "  maestro.owner: x\n  maestro.maturity",
            ),
            "skills/valid-skill/SKILL.md: invalid frontmatter: metadata: duplicate key \
                \"maestro.owner\"",
        ),
    ]);
}

#[test]
fn unknown_keys_and_wrong_types_are_refused() {
    assert_refused(vec![
        (
            "agent metadata field",
            MemoryTree::valid()
                .with("core/agents/invalid.agent.md", INVALID_AGENT)
                .with(
                    "core/agents/invalid.maestro.toml",
                    &MemoryTree::valid().text(SIDECAR),
                ),
            "core/agents/invalid.agent.md: metadata: unknown key",
        ),
        (
            "sidecar key",
            edited(SIDECAR, "maturity", "color = \"red\"\nmaturity"),
            "core/agents/valid.maestro.toml: color: unknown key",
        ),
        (
            "preset key",
            edited(PRESET, "[settings]", "color = \"red\"\n\n[settings]"),
            "presets/knowledge-client.toml: color: unknown key",
        ),
        (
            "skill metadata key",
            edited(
                SKILL,
                "  maestro.maturity",
                "  maestro.color: red\n  maestro.maturity",
            ),
            "skills/valid-skill/SKILL.md: metadata.maestro.color: unknown key",
        ),
        (
            "wrong type",
            edited(
                SIDECAR,
                "rows = [\"chat.M036 objects\"]",
                "rows = \"chat.M036 objects\"",
            ),
            "core/agents/valid.maestro.toml: rows: must be a list of strings",
        ),
        (
            "preset table value",
            edited(PRESET, "tone = \"normal\"", "tone = [\"normal\"]"),
            "presets/knowledge-client.toml: settings.tone: must be a string, number or boolean",
        ),
    ]);
}

#[test]
fn schema_stage_owner_rows_and_workflows_are_checked() {
    assert_refused(vec![
        (
            "schema version",
            edited(SIDECAR, "maestro-source/2", "maestro-source/3"),
            "core/agents/valid.maestro.toml: schema: unsupported schema \"maestro-source/3\"; this \
                checker reads maestro-source/2",
        ),
        (
            "unknown stage",
            edited(SIDECAR, "\"reviewed\"", "\"draft\""),
            "core/agents/valid.maestro.toml: maturity: unknown maturity \"draft\"",
        ),
        (
            "qualified label",
            edited(SIDECAR, "\"reviewed\"", "\"qualified\""),
            "core/agents/valid.maestro.toml: maturity: qualified needs S4 evidence; a source \
                cannot declare it",
        ),
        (
            "empty owner",
            edited(SIDECAR, "\"@synthetic/knowledge\"", "\"  \""),
            "core/agents/valid.maestro.toml: owner: must name an owner",
        ),
        (
            "missing owner",
            edited(SKILL, "  maestro.owner: \"@synthetic/knowledge\"\n", ""),
            "skills/valid-skill/SKILL.md: metadata.maestro.owner: missing",
        ),
        (
            "no row",
            edited(SIDECAR, "[\"chat.M036 objects\"]", "[]"),
            "core/agents/valid.maestro.toml: rows: must name at least one architecture 08 row",
        ),
        (
            "unknown row",
            edited(SIDECAR, "chat.M036 objects", "chat.M999 nothing"),
            "core/agents/valid.maestro.toml: rows: unknown architecture 08 row \"chat.M999 \
                nothing\"",
        ),
        (
            "row twice",
            edited(
                SIDECAR,
                "[\"chat.M036 objects\"]",
                "[\"chat.M036 objects\", \"chat.M036 objects\"]",
            ),
            "core/agents/valid.maestro.toml: rows: lists \"chat.M036 objects\" twice",
        ),
        (
            "unused by any workflow",
            edited(SIDECAR, "[\"ctm-question\"]", "[]"),
            "core/agents/valid.maestro.toml: workflows: must name at least one workflow it serves",
        ),
        (
            "bad workflow name",
            edited(SIDECAR, "\"ctm-question\"", "\"CTM Question\""),
            "core/agents/valid.maestro.toml: workflows: \"CTM Question\" is not a lower-case \
                hyphenated name",
        ),
    ]);
}

#[test]
fn agent_body_needs_the_six_fixed_sections_in_order() {
    assert_refused(vec![
        (
            "missing section",
            edited(
                AGENT,
                "## Boundaries\n\nSynthetic data only; no other tool.\n",
                "",
            ),
            "core/agents/valid.agent.md: body: expected the sections Purpose, Responsibilities, \
                Inputs, Working sequence, Outputs, Boundaries in order; found Purpose, \
                Responsibilities, Inputs, Working sequence, Outputs",
        ),
        (
            "renamed section",
            edited(AGENT, "## Inputs", "## Input"),
            "found Purpose, Responsibilities, Input, Working sequence, Outputs, Boundaries",
        ),
        (
            "empty section",
            edited(AGENT, "One question about the synthetic glossary.\n", ""),
            "core/agents/valid.agent.md: body: section \"Inputs\" is empty",
        ),
        (
            "no frontmatter",
            edited(AGENT, "---\nname", "name"),
            "core/agents/valid.agent.md: no frontmatter between --- lines",
        ),
        (
            "empty skill body",
            edited(
                SKILL,
                "Cite every answer with the evidence passage it rests on.\n",
                "",
            ),
            "skills/valid-skill/SKILL.md: body: is empty",
        ),
        (
            "skill name",
            edited(SKILL, "name: valid-skill", "name: other-skill"),
            "skills/valid-skill/SKILL.md: name: must equal its directory \"valid-skill\", not \
                \"other-skill\"",
        ),
    ]);
}
