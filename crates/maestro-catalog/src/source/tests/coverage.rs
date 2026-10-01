//! Refusals each guard owns alone: tool names and lists, agent sections,
//! folder patterns, embedded metadata tables, settings classes, YAML shapes,
//! unreadable entries and the escaping of what a diagnostic prints.

use super::support::{MemoryTree, assert_refused, check_under};
use crate::limits::Limits;

/// The valid agent profile's path.
const AGENT: &str = "core/agents/valid.agent.md";

/// The valid catalog with `from` replaced by `to` in `path`.
fn edited(path: &str, from: &str, to: &str) -> MemoryTree {
    MemoryTree::valid().edit(path, from, to)
}

/// The valid catalog whose agent lists `tools`.
fn tools(tools: &str) -> MemoryTree {
    edited(AGENT, "tools: [\"view\"]", &format!("tools: {tools}"))
}

#[test]
fn tool_names_and_lists_are_checked() {
    assert_refused(vec![
        (
            "tool not a name",
            tools("[\"maestro/Bad\"]"),
            "core/agents/valid.agent.md: tools: \"maestro/Bad\" is not a tool name",
        ),
        (
            "server not a name",
            tools("[\"Bad/x\"]"),
            "core/agents/valid.agent.md: tools: \"Bad/x\" is not a tool name",
        ),
        (
            "tools as a string",
            tools("view"),
            "core/agents/valid.agent.md: tools: must be a list of strings",
        ),
        (
            "tool twice",
            tools("[\"view\", \"view\"]"),
            "core/agents/valid.agent.md: tools: lists \"view\" twice",
        ),
        (
            "empty tool",
            tools("[\" \"]"),
            "core/agents/valid.agent.md: tools: must not list an empty string",
        ),
        (
            "server tool not a tool name",
            tools("[\"Bad Tool\"]"),
            "core/agents/valid.agent.md: tools: \"Bad Tool\" is not a tool name",
        ),
    ]);
}

#[test]
fn a_section_of_two_paragraphs_holds_text() {
    let tree = edited(
        AGENT,
        "Answer questions about the public synthetic glossary.\n",
        "Answer questions about the public synthetic glossary.\n\nCite the evidence.\n",
    );
    assert!(check_under(&tree, &Limits::PRODUCTION).is_ok());
}

#[test]
fn folders_outside_the_layout_are_refused() {
    let agent = MemoryTree::valid().text(AGENT);
    let skill = MemoryTree::valid().text("skills/valid-skill/SKILL.md");
    assert_refused(vec![
        (
            "folder outside the patterns",
            MemoryTree::valid().with("agents/other/x.agent.md", &agent),
            "agents/other/x.agent.md: not a registered v4 placement; nested/unknown areas and \
            unregistered trees refuse; migrate old or mixed layouts to maestro-source/2",
        ),
        (
            "file above the patterns",
            MemoryTree::valid().with("agents/x.agent.md", &agent),
            "agents/x.agent.md: not a registered v4 placement; nested/unknown areas and \
            unregistered trees refuse; migrate old or mixed layouts to maestro-source/2",
        ),
        (
            "skill folder not a name",
            MemoryTree::valid().with("skills/Bad_Skill/SKILL.md", &skill),
            "skills/Bad_Skill/SKILL.md: not a registered v4 placement",
        ),
        (
            "folder beside the data folders",
            MemoryTree::valid().with("skills/valid-skill/extra/n.md", "# Notes\n"),
            "skills/valid-skill/extra/n.md: not a registered v4 placement; nested/unknown \
            areas and unregistered trees refuse; migrate old or mixed layouts to maestro-source/2",
        ),
        (
            "skill folder without its file",
            MemoryTree::valid().with("skills/empty/README.md", "# Empty\n"),
            "skills/empty: no SKILL.md",
        ),
    ]);
}

#[test]
fn embedded_metadata_must_be_a_table_and_classes_known() {
    let preset = "description = \"Synthetic preset.\"\n";
    assert_refused(vec![
        (
            "no metadata table",
            MemoryTree::valid().with("presets/knowledge-client.toml", preset),
            "presets/knowledge-client.toml: metadata: missing",
        ),
        (
            "metadata not a table",
            MemoryTree::valid().with(
                "presets/knowledge-client.toml",
                &format!("{preset}metadata = \"reviewed\"\n"),
            ),
            "presets/knowledge-client.toml: metadata: must be a table",
        ),
    ]);
}

#[test]
fn yaml_shapes_outside_the_value_model_are_refused() {
    assert_refused(vec![
        (
            "null",
            edited(AGENT, "name: valid\n", "name: valid\nmodel:\n"),
            "core/agents/valid.agent.md: model: must not be empty",
        ),
        (
            "tag",
            edited(AGENT, "name: valid\n", "name: !custom valid\n"),
            "core/agents/valid.agent.md: name: must not carry a YAML tag",
        ),
        (
            "key not a string",
            edited(AGENT, "name: valid\n", "name: valid\n1: one\n"),
            "core/agents/valid.agent.md: has a key that is not a string",
        ),
        (
            "fraction not finite",
            edited(AGENT, "name: valid\n", "name: valid\nmodel: .nan\n"),
            "core/agents/valid.agent.md: model: must be a finite number",
        ),
        (
            "integer past 64 bits",
            edited(
                AGENT,
                "name: valid\n",
                "name: valid\nmodel: 99999999999999999999\n",
            ),
            "core/agents/valid.agent.md: model: must fit a 64-bit signed integer",
        ),
        (
            "integer past 64 bits below zero",
            edited(
                AGENT,
                "name: valid\n",
                "name: valid\nmodel: -99999999999999999999\n",
            ),
            "core/agents/valid.agent.md: model: must fit a 64-bit signed integer",
        ),
        (
            "integer past 63 bits",
            edited(
                AGENT,
                "name: valid\n",
                "name: valid\nmodel: 18446744073709551615\n",
            ),
            "core/agents/valid.agent.md: model: must fit a 64-bit signed integer",
        ),
        (
            "fraction for text",
            edited(AGENT, "name: valid\n", "name: 1.5\n"),
            "core/agents/valid.agent.md: name: must be a nonempty string",
        ),
    ]);
}

#[test]
fn unreadable_files_and_directories_are_reported() {
    assert_refused(vec![
        (
            "file",
            MemoryTree::valid().with_unreadable("core/agents/valid.maestro.toml"),
            "core/agents/valid.maestro.toml: cannot read: permission denied",
        ),
        (
            "directory",
            MemoryTree::valid().with_unreadable("core/agents"),
            "core/agents: cannot list: permission denied",
        ),
    ]);
}

#[test]
fn an_unreadable_file_marks_the_refusal_even_past_the_listed_diagnostics() {
    let tree = (0..1_500)
        .fold(MemoryTree::valid(), |tree, index| {
            tree.with(&format!("core/agents/stray-{index:04}.txt"), "notes\n")
        })
        .with_unreadable("core/agents/valid.maestro.toml");
    let refusal = check_under(&tree, &Limits::PRODUCTION).unwrap_err();
    assert!(refusal.unreadable());
    assert_eq!(refusal.diagnostics.len(), 1_001);
    let readable = MemoryTree::valid().with("core/agents/notes.txt", "notes\n");
    assert!(
        !check_under(&readable, &Limits::PRODUCTION)
            .unwrap_err()
            .unreadable()
    );
}

#[test]
fn diagnostics_escape_control_characters() {
    let tree = MemoryTree::valid().with("core/agents/\u{1b}[31mred.txt", "notes\n");
    let lines: Vec<String> = check_under(&tree, &Limits::PRODUCTION)
        .unwrap_err()
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        lines,
        [
            "core/agents/\\u{1b}[31mred.txt: not a registered v4 placement; nested/unknown \
            areas and unregistered trees refuse; migrate old or mixed layouts to maestro-source/2"
        ]
    );
}

#[test]
fn a_toml_syntax_error_is_one_line_without_the_source() {
    let tree = edited(
        "core/agents/valid.maestro.toml",
        "maturity",
        "owner = \"x\"\nmaturity",
    );
    let lines: Vec<String> = check_under(&tree, &Limits::PRODUCTION)
        .unwrap_err()
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        lines,
        ["core/agents/valid.maestro.toml: invalid TOML at line 3, column 1: duplicate key"]
    );
}
