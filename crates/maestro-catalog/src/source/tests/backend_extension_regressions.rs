//! Selection retains checked defaults and rejects role-specific unsafe neighbours.

use super::{
    backend_extensions::{activation_tree, package},
    support::check_under,
};
use crate::{
    limits::Limits,
    source::{ResourceId, Value, builtin},
};

/// Public backend fixtures stay synthetic and share the production checker.
const VECTOR: &str = include_str!("../../../../../tests/fixtures/catalog/backends/vectordb.toml");
/// MCP has no unchecked server declarations in this slice.
const MCP: &str = include_str!("../../../../../tests/fixtures/catalog/backends/mcp.toml");

#[test]
fn common_ceiling_survives_activation() {
    let tree = activation_tree();
    let graph = tree
        .text("core/backends/graphdb/config.toml")
        .replace("max_num_threads = 2\n", "");
    let tree = package(tree, "alpha", "[\"graphdb\"]")
        .with("core/backends/graphdb/config.toml", &graph)
        .with(
            "settings/defaults.toml",
            "schema = \"maestro-preferences/1\"\n[graphdb]\nmax_num_threads = 4",
        )
        .with(
            "capabilities/team/alpha/backends/graphdb/config.toml",
            "[graphdb]\nmax_num_threads = 3",
        );
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    let view = catalog
        .effective_backend_extensions(
            &[ResourceId::parse("package:alpha").unwrap()],
            &builtin().unwrap(),
        )
        .unwrap();
    assert_eq!(view.ceilings["graphdb.max_num_threads"], 3);
}

#[test]
fn backend_extension_mixed_role_aggregate_refuses() {
    let tree = package(activation_tree(), "alpha", "[\"graphdb\", \"vectordb\"]")
        .with("core/backends/vectordb/config.toml", VECTOR)
        .with(
            "capabilities/team/alpha/backends/graphdb/config.toml",
            "projections = [\"alpha/overview\"]",
        )
        .with(
            "capabilities/team/alpha/backends/vectordb/config.toml",
            "collections = [\"alpha/documents\"]",
        );
    let resources = check_under(&tree, &Limits::PRODUCTION)
        .unwrap()
        .resources
        .len();
    let limits = Limits {
        catalog_resources: resources + 2,
        ..Limits::PRODUCTION
    };
    assert!(check_under(&tree, &limits).is_ok());
    let error = check_under(
        &tree,
        &Limits {
            catalog_resources: resources + 1,
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
fn backend_extension_activation_rechecks_all_role_types() {
    let registry = builtin().unwrap();
    let selected = [ResourceId::parse("package:alpha").unwrap()];
    for (role, base, extension) in [
        ("vectordb", VECTOR, "collections = [\"alpha/documents\"]"),
        ("mcp", MCP, "[mcp]"),
    ] {
        let tree = package(activation_tree(), "alpha", &format!("[\"{role}\"]"))
            .with(&format!("core/backends/{role}/config.toml"), base)
            .with(
                &format!("capabilities/team/alpha/backends/{role}/config.toml"),
                extension,
            );
        let mut catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
        assert!(catalog.selection(&selected, &registry).is_ok());
        catalog
            .resources
            .iter_mut()
            .find(|resource| resource.id.kind == "backend" && resource.id.name == role)
            .unwrap()
            .fields
            .insert("type".to_owned(), Value::Text("ladybug".to_owned()));
        let error = catalog.selection(&selected, &registry).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("incompatible with core backend type"),
            "{role}: {error}"
        );
    }
}

#[test]
fn backend_extension_physical_fields_refuse() {
    for key in [
        "collection",
        "dimensions",
        "lifecycle",
        "credentials",
        "endpoint",
    ] {
        let tree = package(activation_tree(), "alpha", "[\"vectordb\"]")
            .with("core/backends/vectordb/config.toml", VECTOR)
            .with(
                "capabilities/team/alpha/backends/vectordb/config.toml",
                &format!("{key} = \"forbidden\""),
            );
        let error = check_under(&tree, &Limits::PRODUCTION).unwrap_err();
        assert!(
            error.to_string().contains(&format!("{key}: unknown key")),
            "{error}"
        );
    }
}
