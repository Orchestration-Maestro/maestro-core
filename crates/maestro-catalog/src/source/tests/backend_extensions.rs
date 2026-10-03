//! Add-or-narrow declarations use the real bounded checker, never a store.

use super::support::{MemoryTree, check_under};
use crate::source::backend_extensions::effective;
use crate::{limits::Limits, source::Catalog};
use std::collections::BTreeSet;

/// Core graph defaults, with the normal area-derived metadata.
const GRAPH: &str = include_str!("../../../../../tests/fixtures/catalog/backends/graphdb.toml");

/// One package selecting exact owner-local files.
pub(super) fn package(tree: MemoryTree, owner: &str, roles: &str) -> MemoryTree {
    let root = tree
        .text("core/package.toml")
        .replace("\"core\"", &format!("\"{owner}\""));
    tree.with(
        &format!("capabilities/team/{owner}/package.toml"),
        &format!("backend_extensions = {roles}\n{root}"),
    )
}

/// A synthetic owner plus core base, with no adapters or side effects.
fn tree(role: &str, text: &str) -> MemoryTree {
    package(
        MemoryTree::owned().with("core/backends/graphdb/config.toml", GRAPH),
        "alpha",
        &format!("[\"{role}\"]"),
    )
    .with(
        &format!("capabilities/team/alpha/backends/{role}/config.toml"),
        text,
    )
}

/// Every refusal must name its guarded invariant, not merely some other error.
fn refuses(tree: &MemoryTree, reason: &str) {
    let error = check_under(tree, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(error.contains(reason), "expected {reason}: {error}");
}

#[test]
fn backend_extension_narrows() {
    let catalog = check_under(
        &tree(
            "graphdb",
            include_str!("../../../../../tests/fixtures/catalog/backends/graphdb-extension.toml"),
        ),
        &Limits::PRODUCTION,
    )
    .unwrap();
    let package = catalog
        .resources
        .iter()
        .find(|resource| resource.id.name == "alpha")
        .unwrap();
    assert!(
        package
            .data
            .contains(&"capabilities/team/alpha/backends/graphdb/config.toml".to_owned())
    );
}

#[test]
fn backend_replacement_refuses() {
    for (text, reason) in [
        ("type = \"ladybug\"", "type: unknown key"),
        ("base = \"other\"", "base: unknown key"),
        (
            "endpoint = \"https://example.invalid\"",
            "endpoint: unknown key",
        ),
        ("server = \"knowledge\"", "server: unknown key"),
        ("trust = [\"all\"]", "trust: unknown key"),
        (
            "[graphdb]\nmax_num_threads = 3",
            "cannot raise the base ceiling",
        ),
        ("[graphdb]\nroot = 1", "not a narrowable graph control"),
        ("[graphdb]\nmax_db_size = 16777217", "graphdb.max_db_size"),
    ] {
        refuses(&tree("graphdb", text), reason);
    }
}

#[test]
fn extension_aggregate_limit_refuses() {
    let mut tree = tree("graphdb", "projections = [\"alpha/one\", \"alpha/two\"]");
    tree = package(tree, "beta", "[\"graphdb\"]").with(
        "capabilities/team/beta/backends/graphdb/config.toml",
        "projections = [\"beta/one\", \"beta/two\"]",
    );
    // Five area/base resources plus four additions share one resource ceiling.
    let limits = Limits {
        catalog_resources: 9,
        ..Limits::PRODUCTION
    };
    assert!(check_under(&tree, &limits).is_ok());
    let error = check_under(
        &tree,
        &Limits {
            catalog_resources: 8,
            ..limits
        },
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("aggregate backend additions"),
        "{error}"
    );
}

#[test]
fn extension_collision_refuses() {
    for names in [
        "[\"alpha/one\", \"alpha/two\", \"alpha/one\"]",
        "[\"alpha/one\", \"alpha/one\", \"alpha/two\"]",
    ] {
        refuses(
            &tree("graphdb", &format!("projections = {names}")),
            "duplicate backend addition",
        );
    }
    refuses(
        &tree("graphdb", "projections = [\"beta/one\"]"),
        "own package namespace",
    );
    for name in ["one", "alpha/", "alpha/UPPER", "alpha/one/two"] {
        refuses(
            &tree("graphdb", &format!("projections = [\"{name}\"]")),
            "own package namespace",
        );
    }
}

#[test]
fn backend_extension_selection_is_exact() {
    let valid = tree("graphdb", "[graphdb]\nmax_num_threads = 1");
    refuses(
        &valid
            .clone()
            .without("capabilities/team/alpha/backends/graphdb/config.toml"),
        "cannot read inventoried asset",
    );
    refuses(
        &valid.clone().edit(
            "capabilities/team/alpha/package.toml",
            "[\"graphdb\"]",
            "[]",
        ),
        "not a registered v4 placement",
    );
    refuses(
        &valid.clone().edit(
            "capabilities/team/alpha/package.toml",
            "[\"graphdb\"]",
            "[\"graphdb\", \"graphdb\"]",
        ),
        "duplicate backend extension role",
    );
    refuses(
        &valid.clone().edit(
            "capabilities/team/alpha/package.toml",
            "[\"graphdb\"]",
            "[\"unknown\"]",
        ),
        "unknown backend extension role",
    );
    refuses(
        &valid.with("capabilities/team/alpha/backends/graphdb/extra.toml", ""),
        "not a registered v4 placement",
    );
}

#[test]
fn backend_extension_type_changes_revalidate() {
    let valid = tree(
        "graphdb",
        "projections = [\"alpha/one\"]\n[graphdb]\nmax_num_threads = 1",
    );
    assert!(check_under(&valid, &Limits::PRODUCTION).is_ok());
    refuses(
        &valid.edit("core/backends/graphdb/config.toml", "ladybug", "none"),
        "incompatible with core backend type",
    );
}

#[test]
fn backend_extension_shapes_are_role_specific() {
    let vector = tree("vectordb", "collections = [\"alpha/one\"]").with(
        "core/backends/vectordb/config.toml",
        include_str!("../../../../../tests/fixtures/catalog/backends/vectordb.toml"),
    );
    assert!(check_under(&vector, &Limits::PRODUCTION).is_ok());
    refuses(
        &vector.with(
            "capabilities/team/alpha/backends/vectordb/config.toml",
            "projections = []",
        ),
        "projections: unknown key",
    );
    let mcp = tree("mcp", "[mcp]").with(
        "core/backends/mcp/config.toml",
        include_str!("../../../../../tests/fixtures/catalog/backends/mcp.toml"),
    );
    assert!(check_under(&mcp, &Limits::PRODUCTION).is_ok());
    refuses(
        &mcp.with(
            "capabilities/team/alpha/backends/mcp/config.toml",
            "[mcp]\nservers = []",
        ),
        "mcp.servers: unknown key",
    );
}

/// Keep the checker result type explicit for source-named tests added below.
fn checked(tree: &MemoryTree) -> Catalog {
    check_under(tree, &Limits::PRODUCTION).unwrap()
}

#[test]
fn backend_extension_limits_remain_bounded() {
    let valid = tree("graphdb", "[graphdb]\nmax_num_threads = 1");
    assert_eq!(checked(&valid).resources.len(), 4);
    for text in [
        "[graphdb]\nmax_num_threads = 0",
        "[graphdb]\nmax_num_threads = -1",
        "[graphdb]\nmax_num_threads = true",
    ] {
        refuses(&tree("graphdb", text), "graphdb.max_num_threads");
    }
}

/// Selected closure includes canonical Maestro and real package ownership.
pub(super) fn activation_tree() -> MemoryTree {
    let tree = MemoryTree::valid();
    tree.clone()
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
        .with("core/backends/graphdb/config.toml", GRAPH)
}

#[test]
fn backend_extension_minimum_and_removal_are_order_independent() {
    use crate::source::{ResourceId, builtin};
    let tree = package(activation_tree(), "alpha", "[\"graphdb\"]").with(
        "capabilities/team/alpha/backends/graphdb/config.toml",
        "projections = [\"alpha/overview\"]\n[graphdb]\nmax_num_threads = 2",
    );
    let tree = package(tree, "beta", "[\"graphdb\"]").with(
        "capabilities/team/beta/backends/graphdb/config.toml",
        "[graphdb]\nmax_num_threads = 1",
    );
    let registry = builtin().unwrap();
    let alpha = ResourceId::parse("package:alpha").unwrap();
    let beta = ResourceId::parse("package:beta").unwrap();
    let mut catalog = checked(&tree);
    let first = catalog
        .effective_backend_extensions(&[alpha.clone(), beta.clone()], &registry)
        .unwrap();
    assert_eq!(first.ceilings["graphdb.max_num_threads"], 1);
    assert_eq!(first.projections.len(), 1);
    let settings = maestro_settings::Registry::built_in().unwrap();
    let mut resources = catalog.resources.iter().collect::<Vec<_>>();
    let forward = effective(&resources, &settings, &Limits::PRODUCTION).unwrap();
    resources.reverse();
    assert_eq!(
        forward,
        effective(&resources, &settings, &Limits::PRODUCTION).unwrap()
    );
    catalog.resources.reverse();
    let second = catalog
        .effective_backend_extensions(&[beta, alpha.clone()], &registry)
        .unwrap();
    assert_eq!(first, second);
    let remaining = catalog
        .effective_backend_extensions(&[alpha], &registry)
        .unwrap();
    assert_eq!(remaining.ceilings["graphdb.max_num_threads"], 2);
    assert_eq!(remaining.projections, first.projections);
    assert!(
        catalog
            .effective_backend_extensions(&[], &registry)
            .unwrap()
            .ceilings
            .is_empty()
    );
    assert!(
        catalog
            .effective_backend_extensions(&[], &registry)
            .unwrap()
            .projections
            .is_empty()
    );
}

#[test]
fn backend_extension_activation_rechecks_current_core_type() {
    use crate::source::{ResourceId, Value, builtin};
    let tree = package(activation_tree(), "alpha", "[\"graphdb\"]").with(
        "capabilities/team/alpha/backends/graphdb/config.toml",
        "projections = [\"alpha/overview\"]",
    );
    let mut catalog = checked(&tree);
    let registry = builtin().unwrap();
    let selected = [ResourceId::parse("package:alpha").unwrap()];
    let compiled = BTreeSet::from(["ladybug".to_owned()]);
    assert!(
        catalog
            .runtime_selection(&selected, &registry, &compiled)
            .is_ok()
    );
    let base = catalog
        .resources
        .iter_mut()
        .find(|resource| resource.id.kind == "backend")
        .unwrap();
    base.fields
        .insert("type".to_owned(), Value::Text("none".to_owned()));
    let error = catalog
        .runtime_selection(&selected, &registry, &compiled)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("incompatible with core backend type"),
        "{error}"
    );
}

#[test]
fn backend_extension_must_respect_effective_manifest_default() {
    let graph = GRAPH.replace("max_num_threads = 2\n", "");
    let source = tree("graphdb", "[graphdb]\nmax_num_threads = 2")
        .with("core/backends/graphdb/config.toml", &graph)
        .with(
            "settings/defaults.toml",
            "schema = \"maestro-preferences/1\"\n[graphdb]\nmax_num_threads = 1",
        );
    refuses(&source, "cannot raise the base ceiling");
}

#[test]
fn backend_extension_descriptor_class_controls_narrowing() {
    use crate::source::{Known, builtin, check, frozen_rows};
    use maestro_settings::{Registry, SettingClass};
    let registry = Registry::built_in().unwrap();
    let mut descriptors = registry.descriptors().cloned().collect::<Vec<_>>();
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.key == "graphdb.max_num_threads")
        .unwrap()
        .class = SettingClass::Free;
    let settings = Registry::new(&descriptors).unwrap();
    let rows = frozen_rows();
    let known = Known {
        rows: &rows,
        settings: &settings,
        today: 0,
    };
    let error = check(
        &tree("graphdb", "[graphdb]\nmax_num_threads = 1"),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
        known,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("not a narrowable graph control"),
        "{error}"
    );
}

#[test]
fn backend_extension_unsupported_owner_and_missing_base_refuse() {
    let root = MemoryTree::owned();
    let common = root.text("package.toml");
    refuses(
        &root.clone().with(
            "package.toml",
            &format!("backend_extensions = []\n{common}"),
        ),
        "only to optional packages",
    );
    let core = root.text("core/package.toml");
    refuses(
        &root.with(
            "core/package.toml",
            &format!("backend_extensions = []\n{core}"),
        ),
        "only to optional packages",
    );
    refuses(
        &tree("vectordb", "collections = []"),
        "missing core backend base",
    );
    let empty = tree("graphdb", "").edit("core/backends/graphdb/config.toml", "ladybug", "none");
    assert!(check_under(&empty, &Limits::PRODUCTION).is_ok());
}

#[test]
fn backend_extension_input_limits_and_strict_shapes_refuse() {
    let valid = tree("graphdb", "[graphdb]\nmax_num_threads = 1");
    let limits = Limits {
        source_depth: 1,
        ..Limits::PRODUCTION
    };
    assert!(
        check_under(&valid, &limits)
            .unwrap_err()
            .to_string()
            .contains("deeper than")
    );
    for text in [
        "type = 'ladybug'\ntype = 'none'",
        "graphdb = []",
        "graphdb = 1",
        "projections = [1]",
    ] {
        assert!(check_under(&tree("graphdb", text), &Limits::PRODUCTION).is_err());
    }
    refuses(
        &tree("graphdb", "checked-backend-extensions = {}"),
        "checked-backend-extensions: unknown key",
    );
    let oversized = tree(
        "graphdb",
        &"#".repeat(usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap() + 1),
    );
    assert!(
        check_under(&oversized, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string()
            .contains("larger than")
    );
}

#[test]
fn backend_extension_selection_requires_checked_inputs() {
    use crate::source::{ResourceId, Value, builtin};
    let tree = package(activation_tree(), "alpha", "[\"graphdb\"]").with(
        "capabilities/team/alpha/backends/graphdb/config.toml",
        "projections = [\"alpha/overview\"]",
    );
    let original = checked(&tree);
    for role_only in [false, true] {
        let mut catalog = original.clone();
        let owner = catalog
            .resources
            .iter_mut()
            .find(|resource| resource.id.name == "alpha")
            .unwrap();
        if role_only {
            let Value::Table(inputs) = owner.fields.get_mut("checked-backend-extensions").unwrap()
            else {
                panic!("checked inputs are a table");
            };
            inputs.remove("graphdb");
        } else {
            owner.fields.remove("checked-backend-extensions");
        }
        let error = catalog
            .selection(
                &[ResourceId::parse("package:alpha").unwrap()],
                &builtin().unwrap(),
            )
            .unwrap_err();
        assert!(
            error.to_string().contains(if role_only {
                "missing checked backend extension role"
            } else {
                "missing checked backend extension input"
            }),
            "{error}"
        );
    }
}

#[test]
fn backend_extension_noninteger_descriptor_refuses() {
    use crate::source::{Known, builtin, check, frozen_rows};
    use maestro_settings::{Registry, SettingKind};
    let registry = Registry::built_in().unwrap();
    let mut descriptors = registry.descriptors().cloned().collect::<Vec<_>>();
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.key == "graphdb.max_num_threads")
        .unwrap()
        .kind = SettingKind::Number {
        min: 1.0,
        max: 64.0,
        off: false,
    };
    let settings = Registry::new(&descriptors).unwrap();
    let rows = frozen_rows();
    let known = Known {
        rows: &rows,
        settings: &settings,
        today: 0,
    };
    let error = check(
        &tree("graphdb", "[graphdb]\nmax_num_threads = 1"),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
        known,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("not a numeric ceiling"),
        "{error}"
    );
}
