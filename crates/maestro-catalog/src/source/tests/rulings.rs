//! The C03 round-two rulings: skill metadata reads only `maestro.` keys,
//! closures skip roots that are not reviewed, and the agent, MCP and
//! metadata fields the rulings admit.

use super::support::{MemoryTree, assert_refused, check_under};
use crate::limits::Limits;

/// The valid agent profile's path.
const AGENT: &str = "core/agents/valid.agent.md";
/// The valid skill's path.
const SKILL: &str = "skills/valid-skill/SKILL.md";
/// The valid preset's path.
const PRESET: &str = "presets/knowledge-client.toml";

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
            "skills/valid-skill/SKILL.md: metadata.maturity: ambiguous; Maestro reads only \
                maestro.maturity",
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
    assert!(!skill.fields.contains_key("author"));
    assert!(skill.metadata.version.is_none());
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
        "core/agents/valid.maestro.toml",
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
            "core/agents/valid.maestro.toml",
            "maturity",
            "version = \" \"\nmaturity",
        ),
        "core/agents/valid.maestro.toml: version: must be a nonempty string",
    )]);
}

#[test]
fn agents_keep_native_model_and_fail_closed_on_unbound_mcp_names() {
    let tree = edited(AGENT, "tools:", "model: synthetic-model\ntools:");
    let checked = check_under(&tree, &Limits::PRODUCTION).unwrap();
    assert_eq!(
        checked.resources[0].fields["model"].text(),
        Some("synthetic-model")
    );
    assert_refused(vec![
        (
            "unbound server",
            edited(AGENT, "tools:", "mcp-servers: [\"maestro\"]\ntools:"),
            "mcp-servers: names mcp:maestro, which does not exist",
        ),
        (
            "invalid server name",
            edited(AGENT, "tools:", "mcp-servers: [\"Bad Server\"]\ntools:"),
            "mcp-servers: \"Bad Server\" is not a lower-case hyphenated name",
        ),
        (
            "legacy resource",
            MemoryTree::valid().with("mcp/maestro.toml", "args = [\"-v\", \"mcp\", \"-v\"]"),
            "mcp/maestro.toml: not a registered v4 placement",
        ),
    ]);
}
