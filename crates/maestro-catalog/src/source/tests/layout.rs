//! The catalog's layout: agent and sidecar pairing, duplicate IDs, entries
//! that are not resources, unsupported kinds and links.

use super::support::{MemoryTree, assert_refused};

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
            "missing settings classes",
            valid.clone().without("settings/classes.toml"),
            "settings/classes.toml: missing: every setting needs exactly one class",
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
