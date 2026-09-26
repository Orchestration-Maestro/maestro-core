//! Tests of the capability registry: declarations, refusals and order.
//!
//! What a tool declares is what the registry lists, a name registered twice is
//! refused, and tools are listed in name order.

use super::{DuplicateTool, Effect, Registry, Tool};
use serde_json::json;
use std::collections::BTreeSet;

/// A tool named `name` that reads within `scope`, given one query.
fn tool(name: &str, scope: &str) -> Tool {
    Tool {
        name: name.to_owned(),
        schema: json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"],
        }),
        effects: BTreeSet::from([Effect::Read]),
        scopes: BTreeSet::from([scope.to_owned()]),
    }
}

#[test]
fn a_tool_registers_with_its_schema_effects_and_scopes() {
    let mut registry = Registry::default();
    let search = Tool {
        name: "knowledge_search".to_owned(),
        schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 },
            },
            "required": ["query"],
        }),
        effects: BTreeSet::from([Effect::Read, Effect::Network, Effect::Model]),
        scopes: BTreeSet::from([
            "workspace/default/collection/ctm".to_owned(),
            "workspace/default/collection/docs".to_owned(),
        ]),
    };

    registry.register(search.clone()).unwrap();

    assert_eq!(registry.tools().collect::<Vec<_>>(), [&search]);
}

#[test]
fn a_name_registered_twice_is_refused_naming_the_tool() {
    let mut registry = Registry::default();
    let first = tool("knowledge_get", "workspace/default");
    registry.register(first.clone()).unwrap();
    let mut second = tool("knowledge_get", "workspace/default/collection/ctm");
    second.effects.insert(Effect::Write);

    let refused: DuplicateTool = registry.register(second).unwrap_err();

    assert_eq!(refused.name(), "knowledge_get");
    assert_eq!(
        refused.to_string(),
        "a tool named \"knowledge_get\" is already registered"
    );
    assert_eq!(registry.tools().collect::<Vec<_>>(), [&first]);
}

#[test]
fn tools_are_listed_in_name_order_whatever_the_order_they_came_in() {
    let mut registry = Registry::default();
    for name in [
        "knowledge_search",
        "knowledge_ask",
        "knowledge_get",
        "knowledge_collections",
    ] {
        registry.register(tool(name, "workspace/default")).unwrap();
    }

    let names: Vec<&str> = registry.tools().map(|tool| tool.name.as_str()).collect();

    assert_eq!(
        names,
        [
            "knowledge_ask",
            "knowledge_collections",
            "knowledge_get",
            "knowledge_search",
        ]
    );
}
