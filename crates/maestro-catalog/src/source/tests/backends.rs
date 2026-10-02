//! Strict core backend declarations and build-aware activation refusals.

use super::support::{MemoryTree, check_by};
use crate::{
    limits::Limits,
    source::{Catalog, builtin},
};
use std::collections::BTreeSet;

/// Synthetic bases share the production bounded `SourceTree` checker.
const BASES: [(&str, &str); 3] = [
    (
        "graphdb",
        include_str!("../../../../../tests/fixtures/catalog/backends/graphdb.toml"),
    ),
    (
        "vectordb",
        include_str!("../../../../../tests/fixtures/catalog/backends/vectordb.toml"),
    ),
    (
        "mcp",
        include_str!("../../../../../tests/fixtures/catalog/backends/mcp.toml"),
    ),
];

/// Check a role's base with real ownership records.
fn checked(role: &str, text: &str) -> Result<Catalog, String> {
    check_by(
        &MemoryTree::owned().with(&format!("core/backends/{role}/config.toml"), text),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
    )
    .map_err(|error| error.to_string())
}

/// Every refused neighbour must fail for the specific guarded invariant.
fn refuses(role: &str, text: &str, reason: &str) {
    let error = checked(role, text).unwrap_err();
    assert!(error.contains(reason), "expected {reason:?}: {error}");
}

#[test]
fn backend_valid_base_accepts() {
    for (role, text) in BASES {
        let catalog = checked(role, text).unwrap();
        let backend = catalog
            .resources
            .iter()
            .find(|resource| resource.id.kind == "backend")
            .unwrap();
        assert_eq!(backend.id.name, role);
        assert_eq!(backend.id.namespace.as_deref(), Some("core"));
    }
    let none = BASES[0].1.replace("ladybug", "none");
    assert!(checked("graphdb", &none).is_ok());
    for (role, text) in BASES {
        let without_table = text.split("\n\n").next().unwrap().to_owned()
            + "\n\n[metadata]"
            + text.split("[metadata]").nth(1).unwrap();
        assert!(checked(role, &without_table).is_ok());
    }
}

#[test]
fn inactive_backend_table_refuses() {
    let none = BASES[0].1.replace("ladybug", "none");
    refuses(
        "graphdb",
        include_str!("../../../../../tests/fixtures/catalog/backends/inactive-invalid.toml"),
        "graphdb.max_num_threads",
    );
    refuses(
        "graphdb",
        &none.replace("max_num_threads = 2", "unknown = 2"),
        "unknown",
    );
    refuses(
        "graphdb",
        &none.replace("[graphdb]", "[ladybug]"),
        "ladybug: unknown key",
    );
    refuses(
        "graphdb",
        &none.replace("[graphdb]", "[vectordb]"),
        "vectordb",
    );
    let metadata = none.split("[metadata]").nth(1).unwrap();
    refuses(
        "graphdb",
        &format!("type = \"none\"\n[vectordb]\n[metadata]{metadata}"),
        "vectordb: table belongs to another backend role",
    );
    for value in ["[mcp]", "[unknown]"] {
        refuses(
            "graphdb",
            &format!("type = \"none\"\n{value}\n[metadata]{metadata}"),
            if value == "[mcp]" {
                "mcp: table belongs to another backend role"
            } else {
                "unknown: unknown key"
            },
        );
    }
}

#[test]
fn backend_numeric_bounds_refuse() {
    for (key, valid, invalid) in [
        (
            "buffer_pool_size",
            "268435456",
            vec!["0", "16777215", "1073741825", "-1", "18446744073709551616"],
        ),
        (
            "max_db_size",
            "17179869184",
            vec!["0", "8388608", "2199023255552", "16777217", "-1"],
        ),
        ("max_num_threads", "2", vec!["0", "65", "-1"]),
    ] {
        for value in invalid {
            let text = BASES[0]
                .1
                .replace(&format!("{key} = {valid}"), &format!("{key} = {value}"));
            // Overflow refuses at the bounded TOML parser, before the hook.
            let reason = match value {
                "18446744073709551616" => "invalid TOML",
                "-1" => "graphdb",
                _ => key,
            };
            refuses("graphdb", &text, reason);
        }
    }
}

#[test]
fn backend_numeric_boundaries_accept() {
    for (pool, size, threads) in [
        (16_777_216_u64, 16_777_216_u64, 1),
        (1_073_741_824, 1_099_511_627_776, 64),
    ] {
        let text = BASES[0]
            .1
            .replace("268435456", &pool.to_string())
            .replace("17179869184", &size.to_string())
            .replace(
                "max_num_threads = 2",
                &format!("max_num_threads = {threads}"),
            );
        assert!(checked("graphdb", &text).is_ok());
    }
}

#[test]
fn backend_unknown_type_and_locked_fields_refuse() {
    for (role, text) in BASES {
        let kind = text.lines().next().unwrap();
        refuses(
            role,
            &text.replace(kind, "type = \"unknown\""),
            "type: unknown backend type",
        );
        refuses(role, &text.replace(kind, "type = 1"), "type");
    }
    for key in ["root", "reader", "writer", "checkpoint_on_close"] {
        let text = BASES[0]
            .1
            .replace("[graphdb]", &format!("[graphdb]\n{key} = true"));
        refuses("graphdb", &text, key);
    }
    for (role, text) in BASES.into_iter().skip(1) {
        refuses(
            role,
            &text.replace(text.lines().next().unwrap(), "type = \"none\""),
            "type: unknown backend type",
        );
    }
}

#[test]
fn backend_machine_config_and_launch_fields_refuse() {
    for (role, text, key) in [
        ("vectordb", BASES[1].1, "endpoint"),
        ("vectordb", BASES[1].1, "credentials"),
        ("vectordb", BASES[1].1, "collection"),
        ("vectordb", BASES[1].1, "dimensions"),
        ("vectordb", BASES[1].1, "lifecycle"),
        ("mcp", BASES[2].1, "command"),
        ("mcp", BASES[2].1, "env"),
        ("mcp", BASES[2].1, "tools"),
    ] {
        let text = text.replace(
            &format!("[{role}]"),
            &format!("[{role}]\n{key} = \"forbidden\""),
        );
        refuses(role, &text, key);
    }
}

/// A reviewed runtime closure with canonical Maestro, not a native adapter.
fn activation_catalog(role: &str, text: &str) -> Catalog {
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
        .with(&format!("core/backends/{role}/config.toml"), text);
    check_by(&tree, &builtin().unwrap(), &Limits::PRODUCTION).unwrap()
}

#[test]
fn uncompiled_backend_refuses() {
    let catalog = activation_catalog("graphdb", BASES[0].1);
    let registry = builtin().unwrap();
    let compiled = BTreeSet::from(["ladybug".to_owned()]);
    assert!(catalog.selection(&[], &registry).is_ok());
    let members = catalog
        .runtime_selection(&[], &registry, &compiled)
        .unwrap();
    assert!(members.iter().any(|resource| resource.id.kind == "backend"));
    let error = catalog
        .runtime_selection(&[], &registry, &BTreeSet::new())
        .unwrap_err();
    assert!(error.to_string().contains("not compiled"));
    let none = activation_catalog("graphdb", &BASES[0].1.replace("ladybug", "none"));
    assert!(
        none.runtime_selection(&[], &registry, &BTreeSet::new())
            .is_ok()
    );
}

#[test]
fn runtime_selection_checks_vector_mcp_and_backend_maturity() {
    let registry = builtin().unwrap();
    for (role, text) in BASES.into_iter().skip(1) {
        let catalog = activation_catalog(role, text);
        let kind = text.lines().next().unwrap().split('"').nth(1).unwrap();
        let compiled = BTreeSet::from([kind.to_owned()]);
        assert!(catalog.runtime_selection(&[], &registry, &compiled).is_ok());
        assert!(
            catalog
                .runtime_selection(&[], &registry, &BTreeSet::new())
                .unwrap_err()
                .to_string()
                .contains("not compiled")
        );
        let draft = activation_catalog(role, &text.replace("reviewed", "authored"));
        assert!(draft.selection(&[], &registry).is_ok());
        assert!(
            draft
                .runtime_selection(&[], &registry, &compiled)
                .unwrap_err()
                .to_string()
                .contains("needs reviewed maturity")
        );
    }
}

#[test]
fn backend_bad_numeric_types_and_secret_channels_refuse_without_values() {
    for value in ["true", "1.5", "\"literal-secret\"", "{ env = \"TOKEN\" }"] {
        let text = BASES[0]
            .1
            .replace("max_num_threads = 2", &format!("max_num_threads = {value}"));
        let error = checked("graphdb", &text).unwrap_err();
        assert!(
            error.contains("graphdb: must be a table of unsigned integers"),
            "{error}"
        );
        assert!(!error.contains("literal-secret"), "{error}");
    }
    let text = BASES[2]
        .1
        .replace("[mcp]", "[mcp]\nenv = { TOKEN = \"literal-secret\" }");
    let error = checked("mcp", &text).unwrap_err();
    assert!(error.contains("mcp.env: unknown key"), "{error}");
    assert!(!error.contains("literal-secret"), "{error}");
}

#[test]
fn backend_non_table_and_unknown_placement_refuse() {
    let metadata = BASES[0].1.split("[metadata]").nth(1).unwrap();
    for value in ["3", "[]", "[[]]", "[[1, 2]]", "\"text\"", "true"] {
        refuses(
            "graphdb",
            &format!("type = \"none\"\ngraphdb = {value}\n[metadata]{metadata}"),
            "graphdb",
        );
    }
    refuses(
        "graphdb",
        &BASES[0].1.replace("type =", "extra = true\ntype ="),
        "extra: unknown key",
    );
    refuses("other", BASES[0].1, "unknown backend role");
}
