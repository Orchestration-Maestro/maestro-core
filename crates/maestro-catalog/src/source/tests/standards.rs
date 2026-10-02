//! C81a mandatory standards and inert registered machine checks.

use super::{
    area_packages::package_source,
    support::{MemoryTree, check_by, check_under},
};
use crate::{
    limits::Limits,
    source::{KindDescriptor, ResourceId, builtin, builtin_hooks},
};

/// A normative inventory on the existing area envelope.
fn standard(name: &str, rule: &str) -> String {
    package_source("package", name)
        .replace("kind = \"package\"", "kind = \"standard\"")
        .replace("[metadata]", &format!("rules = [\"{rule}\"]\n[metadata]"))
}

/// One inert machine check referencing its own standard's rule.
fn machine_check() -> &'static str {
    "name = \"shape\"\nrules = [\"SEC-001\"]\nvalidator = \"area-package\"\n\
     applicability = [\"all\"]\ninputs = [\"skill:common/valid-skill\"]\n\
     evidence = [\"shape-report\"]\nrefusals = [\"invalid-shape\"]\n\
     [metadata]\nschema = \"maestro-source/2\"\nmaturity = \"reviewed\"\n\
     rows = [\"chat.M036 objects\"]\nworkflows = [\"ctm-question\"]\n\
     requires = [\"skill:common/valid-skill\"]\n"
}

/// Two mandatory standards, one check, and canonical Maestro for selection.
fn tree() -> MemoryTree {
    let base = MemoryTree::valid();
    base.clone()
        .with(
            "core/agents/maestro.agent.md",
            &base
                .text("core/agents/valid.agent.md")
                .replace("name: valid", "name: maestro"),
        )
        .with(
            "core/agents/maestro.maestro.toml",
            &base.text("core/agents/valid.maestro.toml"),
        )
        .with(
            "standards/security/package.toml",
            &standard("security", "SEC-001").replace(
                "requires = []",
                "requires = [\"standard-check:security/shape\"]",
            ),
        )
        .with(
            "standards/quality/package.toml",
            &standard("quality", "QLT-001"),
        )
        .with("standards/security/checks/shape.toml", machine_check())
}

#[test]
fn every_standard_is_pinned_once() {
    let registry = builtin().unwrap();
    let mut loaded = builtin_hooks();
    for registration in registry.registrations() {
        let descriptor: KindDescriptor =
            serde_json::from_slice(&serde_json::to_vec(&registration.descriptor).unwrap()).unwrap();
        loaded.register(descriptor).unwrap();
    }
    let catalog = check_by(&tree(), &loaded, &Limits::PRODUCTION).unwrap();
    let explicit = ["preset:knowledge-client", "standard:security"]
        .map(|text| ResourceId::parse(text).unwrap());
    for selection in [&[][..], &explicit[..]] {
        let members = catalog.selection(selection, &loaded).unwrap();
        for required in [
            "standard:security",
            "standard:quality",
            "standard-check:security/shape",
        ] {
            assert_eq!(
                members
                    .iter()
                    .filter(|member| member.id.to_string() == required)
                    .count(),
                1,
                "{required}"
            );
        }
    }
}

#[test]
fn missing_or_optional_standard_refuses() {
    let valid = tree();
    assert!(check_under(&valid, &Limits::PRODUCTION).is_ok());
    for path in ["package.toml", "presets/knowledge-client.toml"] {
        let missing =
            valid
                .clone()
                .edit(path, "requires = [", "requires = [\"standard:missing\", ");
        let refusal = check_under(&missing, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string();
        assert!(
            refusal.contains("standard:missing, which does not exist"),
            "{refusal}"
        );
    }
    for key in ["optional", "disabled", "mandatory"] {
        let optional = valid.clone().edit(
            "standards/security/package.toml",
            "[metadata]",
            &format!("{key} = true\n[metadata]"),
        );
        let refusal = check_under(&optional, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string();
        assert!(
            refusal.contains(&format!("{key}: unknown key")),
            "{refusal}"
        );
    }
}

#[test]
fn standard_removal_refuses() {
    let valid = tree();
    assert!(check_under(&valid, &Limits::PRODUCTION).is_ok());
    for key in ["optional", "exclude", "remove"] {
        for path in [
            "presets/knowledge-client.toml",
            "standards/security/package.toml",
        ] {
            let removed = valid.clone().with(
                path,
                &format!("{key} = [\"standard:security\"]\n{}", valid.text(path)),
            );
            let refusal = check_under(&removed, &Limits::PRODUCTION)
                .unwrap_err()
                .to_string();
            assert!(
                refusal.contains(&format!("{key}: unknown key")),
                "{refusal}"
            );
        }
    }
}

#[test]
fn duplicate_rule_identity_refuses() {
    let valid = tree();
    assert!(check_under(&valid, &Limits::PRODUCTION).is_ok());
    let duplicate = valid
        .clone()
        .edit("standards/quality/package.toml", "QLT-001", "SEC-001");
    let refusal = check_under(&duplicate, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("duplicate rule identity SEC-001"),
        "{refusal}"
    );
    let duplicate = valid.clone().with(
        "standards/security/checks/second.toml",
        &machine_check().replace("name = \"shape\"", "name = \"second\""),
    );
    let refusal = check_under(&duplicate, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("duplicate rule identity SEC-001"),
        "{refusal}"
    );
    let repeated = valid.edit(
        "standards/security/package.toml",
        "rules = [\"SEC-001\"]",
        "rules = [\"SEC-001\", \"SEC-001\"]",
    );
    assert!(
        check_under(&repeated, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string()
            .contains("lists \"SEC-001\" twice")
    );
}

#[test]
fn machine_check_shape_identity_and_inputs_refuse() {
    let valid = tree();
    assert!(check_under(&valid, &Limits::PRODUCTION).is_ok());
    for (from, to, message) in [
        ("SEC-001", "SEC-999", "not in standard:security inventory"),
        ("area-package", "remote-validator", "unregistered validator"),
        (
            "inputs = [\"skill:common/valid-skill\"]",
            "inputs = [\"standard:missing\"]",
            "standard:missing must be declared in metadata.requires",
        ),
        (
            "inputs = [\"skill:common/valid-skill\"]",
            "inputs = [\"https://example.test/check\"]",
            "typed qualified ID",
        ),
        (
            "requires = [\"skill:common/valid-skill\"]",
            "requires = []",
            "skill:common/valid-skill must be declared in metadata.requires",
        ),
        ("rules = [\"SEC-001\"]", "rules = []", "nonempty"),
        (
            "inputs = [\"skill:common/valid-skill\"]",
            "inputs = []",
            "nonempty",
        ),
        (
            "applicability = [\"all\"]",
            "applicability = []",
            "nonempty",
        ),
        ("evidence = [\"shape-report\"]", "evidence = []", "nonempty"),
        (
            "refusals = [\"invalid-shape\"]",
            "refusals = []",
            "nonempty",
        ),
    ] {
        let changed = valid
            .clone()
            .edit("standards/security/checks/shape.toml", from, to);
        let refusal = check_under(&changed, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string();
        assert!(refusal.contains(message), "{refusal}");
    }
    let missing = valid.clone().without("standards/security/package.toml");
    assert!(check_under(&missing, &Limits::PRODUCTION).is_err());
    let empty = valid.edit(
        "standards/quality/package.toml",
        "rules = [\"QLT-001\"]",
        "rules = []",
    );
    assert!(
        check_under(&empty, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string()
            .contains("nonempty")
    );
}
