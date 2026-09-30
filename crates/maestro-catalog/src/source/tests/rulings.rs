//! The C03 round-two rulings: skill metadata reads only `maestro.` keys,
//! closures skip roots that are not reviewed, and the agent, MCP and
//! metadata fields the rulings admit.

use super::support::{MemoryTree, assert_refused, check_under};
use crate::{
    limits::Limits,
    source::{Known, ResourceId, builtin, check, frozen_rows},
};
use maestro_settings::{
    BUILT_IN, Registry as SettingsRegistry, SettingClass, SettingDescriptor, SettingKind,
};
use std::borrow::Cow;

/// The valid agent profile's path.
const AGENT: &str = "agents/base/valid.agent.md";
/// The valid skill's path.
const SKILL: &str = "skills/valid-skill/SKILL.md";
/// The valid preset's path.
const PRESET: &str = "presets/knowledge-client.toml";
/// The valid MCP server's path.
const MCP: &str = "mcp/maestro.toml";

/// The valid catalog with `from` replaced by `to` in `path`.
fn edited(path: &str, from: &str, to: &str) -> MemoryTree {
    MemoryTree::valid().edit(path, from, to)
}

#[test]
fn bare_skill_metadata_keys_named_like_maestro_keys_are_refused() {
    let bare = MemoryTree::valid().text(SKILL).replace("  maestro.", "  ");
    assert_refused(vec![
        (
            "bare keys only",
            MemoryTree::valid().with(SKILL, &bare),
            "skills/valid-skill/SKILL.md: metadata.owner: ambiguous; Maestro reads only \
                maestro.owner",
        ),
        (
            "bare beside prefixed",
            edited(
                SKILL,
                "  maestro.maturity",
                "  maturity: reviewed\n  maestro.maturity",
            ),
            "skills/valid-skill/SKILL.md: metadata.maturity: ambiguous; Maestro reads only \
                maestro.maturity",
        ),
        (
            "bare stage",
            edited(
                SKILL,
                "  maestro.maturity",
                "  stage: reviewed\n  maestro.maturity",
            ),
            "skills/valid-skill/SKILL.md: metadata.stage: ambiguous; Maestro reads only \
                maestro.stage",
        ),
    ]);
}

#[test]
fn foreign_skill_metadata_keys_are_ignored_never_read() {
    let tree = edited(
        SKILL,
        "  maestro.maturity",
        "  author: someone-else\n  version: \"2.0\"\n  maestro.maturity",
    );
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    let skill = &catalog.resources[5];
    assert_eq!(skill.metadata.owner, "@synthetic/knowledge");
}

#[test]
fn closures_skip_roots_that_are_not_reviewed() {
    let tree = edited(PRESET, "maturity = \"reviewed\"", "maturity = \"authored\"").edit(
        "skills/valid-skill/SKILL.md",
        "maestro.maturity: reviewed",
        "maestro.maturity: placeholder",
    );
    assert!(check_under(&tree, &Limits::PRODUCTION).is_ok());
}

#[test]
fn resources_may_declare_a_version() {
    let tree = edited(
        "agents/base/valid.maestro.toml",
        "maturity",
        "version = \"1.2.0\"\nmaturity",
    )
    .edit(
        SKILL,
        "  maestro.maturity",
        "  maestro.version: \"0.3\"\n  maestro.maturity",
    );
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    assert_eq!(
        catalog.resources[0].metadata.version.as_deref(),
        Some("1.2.0")
    );
    assert_eq!(
        catalog.resources[5].metadata.version.as_deref(),
        Some("0.3")
    );
    assert_refused(vec![(
        "empty version",
        edited(
            "agents/base/valid.maestro.toml",
            "maturity",
            "version = \" \"\nmaturity",
        ),
        "agents/base/valid.maestro.toml: version: must be a nonempty string",
    )]);
}

#[test]
fn agents_name_a_model_and_mcp_servers_by_reference() {
    let tree = edited(
        AGENT,
        "tools:",
        "model: synthetic-model\nmcp-servers: [\"maestro\"]\ntools:",
    );
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    let agent = &catalog.resources[0];
    assert_eq!(agent.fields["model"].text(), Some("synthetic-model"));
    assert_eq!(agent.fields["mcp-servers"].texts(), Some(vec!["maestro"]));
}

#[test]
fn agent_mcp_servers_must_exist_and_enter_closures() {
    let only_agent = |tree: MemoryTree| {
        tree.edit(
            PRESET,
            "requires = [\"agent:valid\", \"mcp:maestro\"]",
            "requires = [\"agent:valid\"]",
        )
        .edit(MCP, "maturity = \"reviewed\"", "maturity = \"authored\"")
    };
    assert_refused(vec![
        (
            "absent server",
            edited(AGENT, "tools:", "mcp-servers: [\"absent\"]\ntools:"),
            "agents/base/valid.agent.md: mcp-servers: names mcp:absent, which does not exist",
        ),
        (
            "not a name",
            edited(AGENT, "tools:", "mcp-servers: [\"Bad Server\"]\ntools:"),
            "agents/base/valid.agent.md: mcp-servers: \"Bad Server\" is not a lower-case \
                hyphenated name",
        ),
        (
            "closure through mcp-servers",
            only_agent(edited(
                AGENT,
                "tools: [\"maestro/knowledge_search\", \"view\"]",
                "mcp-servers: [\"maestro\"]",
            )),
            "presets/knowledge-client.toml: metadata.requires: closure member mcp:maestro is \
                authored; a closure admits only reviewed members",
        ),
        (
            "closure through tools",
            only_agent(MemoryTree::valid()),
            "presets/knowledge-client.toml: metadata.requires: closure member mcp:maestro is \
                authored; a closure admits only reviewed members",
        ),
    ]);
}

#[test]
fn mcp_args_are_an_ordered_list_that_may_repeat() {
    let tree = edited(MCP, "args = [\"mcp\"]", "args = [\"-v\", \"mcp\", \"-v\"]");
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    let id = ResourceId {
        kind: "mcp".to_owned(),
        name: "maestro".to_owned(),
    };
    let server = catalog
        .resources
        .iter()
        .find(|resource| resource.id == id)
        .unwrap();
    assert_eq!(server.fields["args"].texts(), Some(vec!["-v", "mcp", "-v"]));
}

#[test]
fn injected_settings_replace_the_shipped_ones() {
    let mut descriptors = BUILT_IN.to_vec();
    descriptors.push(SettingDescriptor {
        key: Cow::Borrowed("colour"),
        kind: SettingKind::Choice {
            values: Cow::Borrowed(&[Cow::Borrowed("blue")]),
            reserved: Cow::Borrowed(&[]),
        },
        default: Cow::Borrowed("blue"),
        description: Cow::Borrowed("A synthetic setting for the catalog port test."),
        class: SettingClass::Free,
    });
    let settings = SettingsRegistry::new(&descriptors).unwrap();
    let rows = frozen_rows();
    let known = Known {
        rows: &rows,
        settings: &settings,
    };
    let tree = edited(PRESET, "tone = \"normal\"", "colour = \"blue\"");
    let lines: Vec<String> = check(&tree, &builtin().unwrap(), &Limits::PRODUCTION, known)
        .unwrap_err()
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        lines,
        ["settings/classes.toml: classes: setting \"colour\" has no class"]
    );
}
