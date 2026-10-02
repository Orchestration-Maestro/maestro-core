//! C34 selection admission, distinct from partial source checking.

use super::{
    area_packages::package_source,
    support::{MemoryTree, check_under},
};
use crate::{
    limits::Limits,
    source::{Catalog, Maturity, ResourceId, Value, builtin},
};

/// One exact qualified selection ID.
fn id(text: &str) -> ResourceId {
    ResourceId::parse(text).unwrap()
}

/// A source-checked catalog with canonical Maestro and two mandatory standards.
fn catalog() -> Catalog {
    let tree = MemoryTree::valid();
    let tree = tree
        .clone()
        .with(
            "core/agents/maestro.agent.md",
            &tree
                .text("core/agents/valid.agent.md")
                .replace("name: valid", "name: maestro"),
        )
        .with(
            "core/agents/maestro.maestro.toml",
            &tree.text("core/agents/valid.maestro.toml"),
        )
        .with(
            "standards/security/package.toml",
            &package_source("standard", "security"),
        )
        .with(
            "standards/quality/package.toml",
            &package_source("standard", "quality"),
        );
    check_under(&tree, &Limits::PRODUCTION).unwrap()
}

#[test]
fn area_owner_reference_mismatch_refuses() {
    let catalog = catalog();
    for resource in &catalog.resources {
        assert!(catalog.ownership(resource).is_some(), "{}", resource.id);
        let mut mismatched = resource.clone();
        if mismatched.id.namespace.is_some() || mismatched.id.kind == "preset" {
            mismatched.id.namespace = Some("security".to_owned());
        } else {
            mismatched.id.name = "other-area".to_owned();
        }
        assert!(
            catalog.ownership(&mismatched).is_none(),
            "{} at {}",
            mismatched.id,
            mismatched.path
        );
    }
    let mut outside = catalog
        .resources
        .iter()
        .find(|resource| resource.id == id("agent:core/maestro"))
        .unwrap()
        .clone();
    outside.path = "core-extra/agents/maestro.agent.md".to_owned();
    assert!(
        catalog.ownership(&outside).is_none(),
        "area directory matching must preserve boundaries"
    );
    let core = catalog
        .resources
        .iter()
        .find(|resource| resource.id == id("package:core"))
        .unwrap();
    assert_eq!(
        catalog.ownership(core).unwrap().owners,
        ["@synthetic/knowledge"]
    );
    let mut multiple = catalog.clone();
    let core = multiple
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("package:core"))
        .unwrap();
    core.fields.insert(
        "owners".to_owned(),
        Value::List(vec![
            Value::Text("first-owner".to_owned()),
            Value::Text("second-owner".to_owned()),
        ]),
    );
    let core = core.clone();
    assert_eq!(multiple.ownership(&core).unwrap().owners.len(), 2);
}

#[test]
fn overlapping_area_ownership_refuses() {
    let mut catalog = catalog();
    let registry = builtin().unwrap();
    let member = catalog
        .resources
        .iter()
        .find(|resource| resource.id == id("agent:core/maestro"))
        .unwrap()
        .clone();
    assert!(catalog.ownership(&member).is_some());
    assert!(catalog.selection(&[], &registry).is_ok());
    let mut overlapping = catalog
        .resources
        .iter()
        .find(|resource| resource.id == id("package:core"))
        .unwrap()
        .clone();
    overlapping.id = id("language:core");
    overlapping.path = "core/agents/package.toml".to_owned();
    catalog.resources.push(overlapping);
    assert!(catalog.ownership(&member).is_none());
    assert!(
        catalog
            .selection(&[], &registry)
            .unwrap_err()
            .to_string()
            .contains("agent:core/maestro does not match its area ownership record")
    );
}

#[test]
fn mandatory_roots_selected_once() {
    let catalog = catalog();
    let registry = builtin().unwrap();
    let explicit = [
        "preset:knowledge-client",
        "package:common",
        "package:core",
        "standard:security",
    ];
    let selected = catalog.selection(&explicit.map(id), &registry).unwrap();
    let selected_ids: Vec<_> = selected
        .iter()
        .map(|resource| resource.id.to_string())
        .collect();
    assert_eq!(
        selected_ids,
        [
            "agent:core/maestro",
            "agent:core/valid",
            "instructions:core/valid",
            "package:common",
            "package:core",
            "preset:knowledge-client",
            "skill:common/valid-skill",
            "standard:quality",
            "standard:security",
        ]
    );
    let repeated = catalog
        .selection(&[id("package:core"), id("package:core")], &registry)
        .unwrap_err();
    assert!(
        repeated
            .to_string()
            .contains("duplicate selection package:core")
    );
    let mut duplicate = catalog.clone();
    duplicate.resources.push(
        catalog
            .resources
            .iter()
            .find(|resource| resource.id == id("standard:security"))
            .unwrap()
            .clone(),
    );
    assert!(
        duplicate
            .selection(&[], &registry)
            .unwrap_err()
            .to_string()
            .contains("duplicate ID standard:security")
    );
    let tree = MemoryTree::valid().edit(
        "presets/knowledge-client.toml",
        "agent:core/valid",
        "agent:core/valid\", \"agent:core/valid",
    );
    assert!(
        check_under(&tree, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string()
            .contains("lists \"agent:core/valid\" twice")
    );
}

#[test]
fn missing_reviewed_maestro_refuses() {
    let catalog = catalog();
    let registry = builtin().unwrap();
    assert!(catalog.selection(&[], &registry).is_ok());
    for required in ["package:common", "package:core", "agent:core/maestro"] {
        let mut missing = catalog.clone();
        missing
            .resources
            .retain(|resource| resource.id != id(required));
        let refusal = missing.selection(&[], &registry).unwrap_err().to_string();
        assert!(
            refusal.contains(&format!("{required} does not exist")),
            "{refusal}"
        );
    }
    for required in [
        "package:common",
        "package:core",
        "standard:security",
        "standard:quality",
        "agent:core/maestro",
    ] {
        for maturity in [Maturity::Placeholder, Maturity::Authored, Maturity::Retired] {
            let mut unreviewed = catalog.clone();
            unreviewed
                .resources
                .iter_mut()
                .find(|resource| resource.id == id(required))
                .unwrap()
                .metadata
                .maturity = maturity;
            let refusal = unreviewed
                .selection(&[], &registry)
                .unwrap_err()
                .to_string();
            assert!(
                refusal.contains(&format!("{required} needs reviewed maturity")),
                "{refusal}"
            );
        }
    }
}

#[test]
fn selected_closure_refuses_missing_unreviewed_and_mismatched_members() {
    let catalog = catalog();
    let registry = builtin().unwrap();
    assert!(
        catalog
            .selection(&[], &registry)
            .unwrap()
            .iter()
            .any(|resource| resource.id == id("skill:common/valid-skill"))
    );
    let mut missing = catalog.clone();
    missing
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("skill:common/valid-skill"))
        .unwrap()
        .metadata
        .requires
        .push(id("skill:common/missing"));
    assert!(
        missing
            .selection(&[], &registry)
            .unwrap_err()
            .to_string()
            .contains("skill:common/missing does not exist")
    );
    let mut authored = catalog.clone();
    authored
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("skill:common/valid-skill"))
        .unwrap()
        .metadata
        .maturity = Maturity::Authored;
    assert!(
        authored
            .selection(&[], &registry)
            .unwrap_err()
            .to_string()
            .contains("skill:common/valid-skill needs reviewed maturity")
    );
    let mut mismatch = catalog.clone();
    mismatch
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("skill:common/valid-skill"))
        .unwrap()
        .path = "core/skills/valid-skill/SKILL.md".to_owned();
    assert!(
        mismatch
            .selection(&[], &registry)
            .unwrap_err()
            .to_string()
            .contains("ownership")
    );
}

#[test]
fn selection_refuses_malformed_and_unregistered_member_ids() {
    let catalog = catalog();
    let registry = builtin().unwrap();
    assert!(catalog.selection(&[], &registry).is_ok());
    let mut malformed = catalog.clone();
    malformed
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("standard:security"))
        .unwrap()
        .id
        .name = "Security".to_owned();
    assert!(
        malformed
            .selection(&[], &registry)
            .unwrap_err()
            .to_string()
            .contains("standard:Security is not a registered typed qualified ID")
    );
    let mut unregistered = super::super::builtin_hooks();
    for registration in registry
        .registrations()
        .filter(|registration| registration.descriptor.kind != "standard")
    {
        unregistered
            .register(registration.descriptor.clone())
            .unwrap();
    }
    assert!(
        catalog
            .selection(&[], &unregistered)
            .unwrap_err()
            .to_string()
            .contains("standard:security is not a registered typed qualified ID")
    );
}

#[test]
fn selection_walks_registered_hook_edges_and_shared_dependencies() {
    let catalog = catalog();
    let registry = builtin().unwrap();
    let expected: Vec<_> = catalog
        .selection(&[], &registry)
        .unwrap()
        .iter()
        .map(|resource| resource.id.clone())
        .collect();
    let mut shared = catalog.clone();
    shared.resources.reverse();
    shared
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("skill:common/valid-skill"))
        .unwrap()
        .metadata
        .requires
        .push(id("skill:common/valid-skill"));
    let actual: Vec<_> = shared
        .selection(&[id("agent:core/maestro")], &registry)
        .unwrap()
        .iter()
        .map(|resource| resource.id.clone())
        .collect();
    assert_eq!(
        actual, expected,
        "visited-ID traversal bounds cycles and deduplicates shared dependencies"
    );
    let mut missing = catalog.clone();
    missing
        .resources
        .iter_mut()
        .find(|resource| resource.id == id("agent:core/maestro"))
        .unwrap()
        .fields
        .insert(
            "mcp-servers".to_owned(),
            Value::List(vec![Value::Text("missing".to_owned())]),
        );
    assert!(
        missing
            .selection(&[], &registry)
            .unwrap_err()
            .to_string()
            .contains("mcp:missing does not exist")
    );
}

#[test]
fn common_cannot_own_a_moved_resource_in_a_missing_area() {
    let catalog = catalog();
    let registry = builtin().unwrap();
    let original = catalog
        .resources
        .iter()
        .find(|resource| resource.id == id("skill:common/valid-skill"))
        .unwrap();
    assert_eq!(
        catalog.ownership(original).unwrap().descriptor.path,
        "package.toml"
    );
    assert!(catalog.selection(&[], &registry).is_ok());
    for path in [
        "capabilities/practice/moved/skills/valid-skill/SKILL.md",
        "languages/rust/skills/valid-skill/SKILL.md",
        "standards/other/skills/valid-skill/SKILL.md",
    ] {
        let mut moved = catalog.clone();
        let resource = moved
            .resources
            .iter_mut()
            .find(|resource| resource.id == id("skill:common/valid-skill"))
            .unwrap();
        resource.path = path.to_owned();
        let resource = resource.clone();
        assert!(
            catalog.ownership(&resource).is_none(),
            "common must not inherit ownership of missing area at {path}"
        );
        assert!(
            moved.selection(&[], &registry).is_err(),
            "selection must refuse a common ID physically moved into a different area: {path}"
        );
    }
}

#[test]
fn retired_area_cannot_enter_a_new_selection() {
    let registry = builtin().unwrap();
    let original = MemoryTree::valid();
    let tree = original
        .clone()
        .with(
            "core/agents/maestro.agent.md",
            &original
                .text("core/agents/valid.agent.md")
                .replace("name: valid", "name: maestro"),
        )
        .with(
            "core/agents/maestro.maestro.toml",
            &original.text("core/agents/valid.maestro.toml"),
        );
    for status in ["active", "deprecated", "retired"] {
        let changed = tree.clone().edit(
            "core/package.toml",
            "status = \"active\"",
            &format!("status = \"{status}\""),
        );
        let checked = check_under(&changed, &Limits::PRODUCTION).unwrap();
        let result = checked.selection(&[], &registry);
        assert_eq!(
            result.is_ok(),
            status != "retired",
            "new selection of source-checked reviewed core with status {status}: {result:?}"
        );
        if status == "retired" {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("agent:core/maestro belongs to retired area package:core")
            );
        }
    }
}

#[test]
fn selected_reverse_dependency_keeps_backend_extension_owner() {
    use super::backend_extensions::{activation_tree, package};
    let tree = package(activation_tree(), "alpha", "[\"graphdb\"]").with(
        "capabilities/team/alpha/backends/graphdb/config.toml",
        "projections = [\"alpha/overview\"]",
    );
    let tree = package(tree, "beta", "[]").edit(
        "capabilities/team/beta/package.toml",
        "requires = []",
        "requires = [\"package:alpha\"]",
    );
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    let view = catalog
        .effective_backend_extensions(&[id("package:beta")], &builtin().unwrap())
        .unwrap();
    assert!(view.projections.contains("alpha/overview"));
    let missing = tree
        .without("capabilities/team/alpha/package.toml")
        .without("capabilities/team/alpha/backends/graphdb/config.toml");
    let error = check_under(&missing, &Limits::PRODUCTION).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("names package:alpha, which does not exist"),
        "{error}"
    );
}
