//! Data-only v4 fixtures; builtin migration belongs to C31/C32.

use super::{registry::glossary, support::MemoryTree};
use crate::{
    limits::Limits,
    source::{
        KindDescriptor, Registry,
        walk::{Found, walk},
    },
};
use serde_json::{Value, json};

/// A descriptor scoped using its serialized source data.
pub(super) fn scoped(directory: &str, scopes: &[&str]) -> KindDescriptor {
    let mut data = serde_json::to_value(glossary()).unwrap();
    data["directory"] = json!(directory);
    data["scopes"] = json!(scopes);
    let parsed = serde_json::from_value(data);
    assert!(parsed.is_ok(), "v4 descriptor must deserialize: {parsed:?}");
    parsed.unwrap()
}

/// A registry containing one scoped descriptor.
pub(super) fn registry() -> Registry {
    let mut registry = Registry::default();
    registry
        .register(scoped(
            "glossaries",
            &["common", "core", "team", "language", "standard"],
        ))
        .unwrap();
    registry
}

/// Discovery only, so a layout guard cannot be masked by content validation.
pub(super) fn discover(tree: &MemoryTree, registry: &Registry, limits: &Limits) -> Found {
    walk(tree, registry, limits).unwrap()
}

/// The exact discovery diagnostic required at `path`.
pub(super) fn refuses(tree: &MemoryTree, registry: &Registry, path: &str, message: &str) {
    let found = discover(tree, registry, &Limits::PRODUCTION);
    assert!(
        found
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == path && diagnostic.message.contains(message)),
        "{:#?}",
        found.diagnostics
    );
}

/// One explicitly inventoried folder resource, described as data.
pub(super) fn folder(assets: &[&str]) -> KindDescriptor {
    let mut data = serde_json::to_value(scoped("skills", &["common"])).unwrap();
    data["layout"] = json!({"folder": {"file": "SKILL.md", "data": assets}});
    data["metadata"] = json!({"table": {"key": "metadata"}});
    data["fields"] = Value::Array(Vec::new());
    serde_json::from_value(data).unwrap()
}
