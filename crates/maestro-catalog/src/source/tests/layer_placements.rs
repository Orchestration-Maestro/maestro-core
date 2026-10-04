//! Registered placements retain their dependency layer outside canonical folders.

use super::support::check_under;
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
        let tree = MemoryTree::owned().with(&path, &source).with(SKILL, &skill);
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
            MemoryTree::owned().with(&path, &preset).with(SKILL, &skill),
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

/// A cross-area skill edge, with a same-layer neighbour using the same local name.
fn layer_neighbours(from: &str, to: &str, namespace: &str) {
    let source = if from.is_empty() {
        SKILL.to_owned()
    } else {
        format!("{from}/{SKILL}")
    };
    let target = format!(
        "{}skills/target/SKILL.md",
        if to.is_empty() {
            String::new()
        } else {
            format!("{to}/")
        }
    );
    let skill = MemoryTree::valid().text(SKILL);
    let tree = MemoryTree::owned()
        .language("python")
        .language("rust")
        .with(
            &source,
            &skill.replace(
                "  maestro.workflows:",
                &format!("  maestro.requires: skill:{namespace}/target\n  maestro.workflows:"),
            ),
        )
        .with(&target, &skill.replace("valid-skill", "target"));
    assert!(check_under(&tree, &Limits::PRODUCTION).is_ok());
    let core_target = "core/skills/target/SKILL.md";
    let refused = tree
        .clone()
        .without(&target)
        .with(core_target, &skill.replace("valid-skill", "target"));
    // The caller supplies the refused layer by moving the target and its qualified ID.
    let refused = refused.edit(
        &source,
        &format!("skill:{namespace}/target"),
        "skill:core/target",
    );
    let result = check_under(&refused, &Limits::PRODUCTION);
    assert!(result.is_err(), "cross-layer edge must refuse");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("layer may not require skill:core/target")
    );
}

#[test]
fn common_to_core_refuses() {
    layer_neighbours("", "", "common");
    let tree = area(
        MemoryTree::default(),
        "standard",
        "security",
        "standards/security/package.toml",
        &["package:core"],
    );
    let tree = area(tree, "package", "core", "core/package.toml", &[]);
    let result = check_under(&tree, &Limits::PRODUCTION);
    assert!(result.is_err(), "standard to core must refuse");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("layer may not require package:core")
    );
}

/// A core/language skill can require a global skill but cannot require a team skill.
fn team_neighbours(from: &str) {
    let skill = MemoryTree::valid().text(SKILL);
    let source = format!("{from}/{SKILL}");
    let tree = MemoryTree::owned()
        .language("rust")
        .with(
            "capabilities/practice/review/package.toml",
            &package_source("package", "review"),
        )
        .with(SKILL, &skill)
        .with(
            &source,
            &skill.replace(
                "  maestro.workflows:",
                "  maestro.requires: skill:common/valid-skill\n  maestro.workflows:",
            ),
        );
    assert!(check_under(&tree, &Limits::PRODUCTION).is_ok());
    let tree = tree
        .with(
            "capabilities/practice/review/skills/valid-skill/SKILL.md",
            &skill,
        )
        .edit(
            &source,
            "skill:common/valid-skill",
            "skill:review/valid-skill",
        );
    let result = check_under(&tree, &Limits::PRODUCTION);
    assert!(result.is_err(), "team edge must refuse");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("layer may not require skill:review/valid-skill")
    );
}

#[test]
fn core_to_team_refuses() {
    team_neighbours("core");
}

#[test]
fn language_to_team_refuses() {
    team_neighbours("languages/rust");
    layer_neighbours("languages/rust", "languages/python", "python");
}
