//! The valid catalog passes, and its typed resources hold exactly what the
//! sources declare.

use super::support::{MemoryTree, check_under};
use crate::{
    limits::Limits,
    source::{Maturity, Metadata, ResourceId, Value, frozen_rows, shipped_settings},
};
use std::{collections::BTreeSet, fs, path::Path};

/// The ID `kind:name`.
fn id(kind: &str, name: &str) -> ResourceId {
    ResourceId {
        kind: kind.to_owned(),
        name: name.to_owned(),
    }
}

/// The text value `text`.
fn text(text: &str) -> Value {
    Value::Text(text.to_owned())
}

#[test]
fn valid_catalog_passes_with_every_resource_sorted_by_id() {
    let catalog = check_under(&MemoryTree::valid(), &Limits::PRODUCTION).unwrap();
    let ids: Vec<String> = catalog
        .resources
        .iter()
        .map(|resource| resource.id.to_string())
        .collect();
    assert_eq!(
        ids,
        [
            "agent:valid",
            "instructions:valid",
            "mcp:maestro",
            "preset:knowledge-client",
            "settings:classes",
            "skill:valid-skill",
        ]
    );
}

#[test]
fn valid_agent_round_trips_its_profile_and_sidecar() {
    let catalog = check_under(&MemoryTree::valid(), &Limits::PRODUCTION).unwrap();
    let agent = &catalog.resources[0];
    assert_eq!(agent.path, "agents/base/valid.agent.md");
    assert_eq!(
        agent.metadata,
        Metadata {
            owner: "@synthetic/knowledge".to_owned(),
            maturity: Maturity::Reviewed,
            rows: vec!["chat.M036 objects".to_owned()],
            workflows: vec!["ctm-question".to_owned()],
            requires: vec![id("skill", "valid-skill"), id("instructions", "valid")],
            version: None,
        }
    );
    assert_eq!(agent.fields["name"], text("valid"));
    assert_eq!(
        agent.fields["description"],
        text("Synthetic agent that answers from the public synthetic glossary.")
    );
    assert_eq!(
        agent.fields["tools"],
        Value::List(vec![text("maestro/knowledge_search"), text("view")])
    );
    assert_eq!(agent.fields.len(), 3);
}

#[test]
fn valid_skill_reads_its_maestro_metadata_strings() {
    let catalog = check_under(&MemoryTree::valid(), &Limits::PRODUCTION).unwrap();
    let skill = &catalog.resources[5];
    assert_eq!(skill.metadata.rows, ["chat.M019 descriptor"]);
    assert_eq!(skill.metadata.owner, "@synthetic/knowledge");
    assert!(skill.metadata.requires.is_empty());
    assert!(!skill.fields.contains_key("metadata"));
}

#[test]
fn skill_lists_split_on_semicolons_and_trim() {
    let tree = MemoryTree::valid().edit(
        "skills/valid-skill/SKILL.md",
        "maestro.rows: chat.M019 descriptor",
        "maestro.rows: \"chat.M019 descriptor; delivery.C04 readiness, owners, dependencies \
            without invalid frontmatter\"",
    );
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    assert_eq!(
        catalog.resources[5].metadata.rows,
        [
            "chat.M019 descriptor",
            "delivery.C04 readiness, owners, dependencies without invalid frontmatter",
        ]
    );
}

#[test]
fn valid_preset_and_classes_keep_their_values() {
    let catalog = check_under(&MemoryTree::valid(), &Limits::PRODUCTION).unwrap();
    let Value::Table(settings) = &catalog.resources[3].fields["settings"] else {
        panic!("no settings table: {:?}", catalog.resources[3]);
    };
    assert_eq!(settings["language"], text("en"));
    assert_eq!(settings["routing_candidates"], Value::Integer(3));
    let Value::Table(classes) = &catalog.resources[4].fields["classes"] else {
        panic!("no classes table: {:?}", catalog.resources[4]);
    };
    let classified: usize = classes
        .values()
        .map(|keys| keys.texts().unwrap().len())
        .sum();
    assert_eq!(classified, shipped_settings().len());
    assert_eq!(
        classes["free"],
        Value::List(vec![text("language"), text("tone")])
    );
}

#[test]
fn discovered_placeholder_outside_every_closure_passes() {
    let valid = MemoryTree::valid();
    let tree = valid
        .clone()
        .with(
            "skills/draft/SKILL.md",
            &valid.text("skills/valid-skill/SKILL.md"),
        )
        .edit("skills/draft/SKILL.md", "name: valid-skill", "name: draft")
        .edit(
            "skills/draft/SKILL.md",
            "maestro.maturity: reviewed",
            "maestro.maturity: placeholder",
        );
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    assert_eq!(catalog.resources[5].id, id("skill", "draft"));
    assert_eq!(
        catalog.resources[5].metadata.maturity,
        Maturity::Placeholder
    );
}

#[test]
fn skill_scripts_and_references_are_data_the_checker_skips() {
    let tree = MemoryTree::valid()
        .with(
            "skills/valid-skill/scripts/run.sh",
            "#!/bin/sh\ntouch ran\n",
        )
        .with("skills/valid-skill/references/notes.md", "# Notes\n");
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    assert_eq!(
        catalog.resources[5].data,
        [
            "skills/valid-skill/references",
            "skills/valid-skill/scripts"
        ]
    );
}

#[test]
fn each_resource_lists_the_files_it_owns() {
    let catalog = check_under(&MemoryTree::valid(), &Limits::PRODUCTION).unwrap();
    let files: Vec<(&str, &[String], usize)> = catalog
        .resources
        .iter()
        .map(|resource| {
            (
                resource.id.kind.as_str(),
                resource.files.as_slice(),
                resource.data.len(),
            )
        })
        .collect();
    assert_eq!(
        files,
        [
            (
                "agent",
                &[
                    "agents/base/valid.agent.md".to_owned(),
                    "agents/base/valid.maestro.toml".to_owned()
                ][..],
                0
            ),
            (
                "instructions",
                &[
                    "instructions/valid.instructions.md".to_owned(),
                    "instructions/valid.maestro.toml".to_owned()
                ][..],
                0
            ),
            ("mcp", &["mcp/maestro.toml".to_owned()][..], 0),
            (
                "preset",
                &["presets/knowledge-client.toml".to_owned()][..],
                0
            ),
            ("settings", &["settings/classes.toml".to_owned()][..], 0),
            ("skill", &["skills/valid-skill/SKILL.md".to_owned()][..], 0),
        ]
    );
}

#[test]
fn frozen_rows_equal_the_traceability_inventory() {
    let inventory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/003-catalog/traceability.json");
    let inventory: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(inventory).unwrap()).unwrap();
    let rows: BTreeSet<String> = inventory["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["source_row"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(rows.len(), 85);
    assert_eq!(frozen_rows(), rows);
}
