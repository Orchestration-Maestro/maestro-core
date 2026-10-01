//! Registered placements retain their dependency layer outside canonical folders.

use super::{area_packages::package_source, references::area, support::MemoryTree};
use crate::limits::Limits;

/// The valid common skill.
const SKILL: &str = "skills/valid-skill/SKILL.md";
/// The canonical preset source.
const PRESET: &str = "presets/knowledge-client.toml";

#[test]
fn root_support_under_presets_cannot_require_team() {
    use super::{registry::glossary, support::check_by};
    use crate::source::{Scope, builtin, builtin_hooks};
    let skill = MemoryTree::valid().text(SKILL);
    for root in ["docs", "presets"] {
        let mut registry = builtin_hooks();
        for registration in builtin().unwrap().registrations() {
            if registration.descriptor.kind != "preset" {
                registry.register(registration.descriptor.clone()).unwrap();
            }
        }
        let mut descriptor = glossary();
        descriptor.directory = format!("{root}/catalog");
        descriptor.scopes = vec![Scope::Root];
        descriptor.requires = vec!["*".to_owned()];
        registry.register(descriptor).unwrap();
        let source = format!(
            "term = \"Evidence\"\n[metadata]{}",
            package_source("package", "common")
                .split_once("[metadata]")
                .unwrap()
                .1
        )
        .replace("requires = []", "requires = [\"skill:common/valid-skill\"]");
        let path = format!("{root}/catalog/evidence.toml");
        let tree = MemoryTree::default()
            .with(&path, &source)
            .with(SKILL, &skill);
        let checked = check_by(&tree, &registry, &Limits::PRODUCTION).unwrap();
        assert!(
            checked
                .resources
                .iter()
                .any(|resource| resource.id.to_string() == "glossary:common/evidence")
        );
        let refused = tree
            .with(
                "capabilities/practice/review/skills/target/SKILL.md",
                &skill.replace("valid-skill", "target"),
            )
            .edit(&path, "skill:common/valid-skill", "skill:review/target");
        let result = check_by(&refused, &registry, &Limits::PRODUCTION);
        assert!(
            result.is_err(),
            "Root support is common, never a preset: {root}: {result:?}"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("layer may not require skill:review/target")
        );
    }
}

#[test]
fn area_cannot_require_registered_preset_under_docs() {
    use super::support::check_by;
    use crate::source::{builtin, builtin_hooks};
    let skill = MemoryTree::valid().text(SKILL);
    let preset = MemoryTree::valid()
        .edit(PRESET, "requires = [\"agent:core/valid\"]", "requires = []")
        .text(PRESET);
    for directory in ["presets", "docs/presets"] {
        let mut registry = builtin_hooks();
        for registration in builtin().unwrap().registrations() {
            let mut descriptor = registration.descriptor.clone();
            if descriptor.kind == "preset" {
                descriptor.directory = directory.to_owned();
            }
            registry.register(descriptor).unwrap();
        }
        let path = format!("{directory}/knowledge-client.toml");
        let tree = area(
            MemoryTree::default()
                .with(&path, &preset)
                .with(SKILL, &skill),
            "language",
            "rust",
            "languages/rust/package.toml",
            &["skill:common/valid-skill"],
        );
        assert!(check_by(&tree, &registry, &Limits::PRODUCTION).is_ok());
        let refused = tree.edit(
            "languages/rust/package.toml",
            "skill:common/valid-skill",
            "preset:knowledge-client",
        );
        let result = check_by(&refused, &registry, &Limits::PRODUCTION);
        assert!(
            result.is_err(),
            "no area may require a preset: {directory}: {result:?}"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("layer may not require preset:knowledge-client")
        );
    }
}
