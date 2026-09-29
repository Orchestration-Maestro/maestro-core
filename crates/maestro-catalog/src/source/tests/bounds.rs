//! D2's source limits at small injected values: each exact boundary passes
//! and one byte, level or resource past it is refused.

use super::support::{MemoryTree, VALID, check_under};
use crate::limits::Limits;

/// The lines of the refusal of the valid catalog under `limits`, or none.
fn refusal_under(limits: &Limits) -> Vec<String> {
    check_under(&MemoryTree::valid(), limits).map_or_else(
        |refusal| {
            refusal
                .diagnostics
                .iter()
                .map(ToString::to_string)
                .collect()
        },
        |_| Vec::new(),
    )
}

#[test]
fn source_file_bytes_boundary() {
    let largest = VALID
        .iter()
        .filter(|(path, _)| !path.starts_with("bootstrap/") && path.contains('/'))
        .max_by_key(|(_, text)| text.len())
        .unwrap();
    let bytes = u64::try_from(largest.1.len()).unwrap();
    let exact = Limits {
        source_file_bytes: bytes,
        ..Limits::PRODUCTION
    };
    assert_eq!(refusal_under(&exact), Vec::<String>::new());
    let past = Limits {
        source_file_bytes: bytes - 1,
        ..Limits::PRODUCTION
    };
    assert_eq!(
        refusal_under(&past),
        [format!("{}: larger than {} bytes", largest.0, bytes - 1)]
    );
}

#[test]
fn source_depth_boundary() {
    let exact = Limits {
        source_depth: 3,
        ..Limits::PRODUCTION
    };
    assert_eq!(refusal_under(&exact), Vec::<String>::new());
    let past = Limits {
        source_depth: 2,
        ..Limits::PRODUCTION
    };
    assert_eq!(
        refusal_under(&past),
        [
            "mcp/maestro.toml: deeper than 2 levels",
            "presets/knowledge-client.toml: deeper than 2 levels",
            "settings/classes.toml: deeper than 2 levels",
        ]
    );
}

#[test]
fn yaml_frontmatter_depth_boundary() {
    let tree = MemoryTree::valid().edit(
        "agents/base/valid.agent.md",
        "tools: [\"maestro/knowledge_search\", \"view\"]",
        "tools: [[\"maestro/knowledge_search\"]]",
    );
    let past = Limits {
        source_depth: 2,
        ..Limits::PRODUCTION
    };
    let lines = check_under(&tree, &past).unwrap_err().diagnostics;
    assert!(
        lines
            .iter()
            .any(|line| line.to_string() == "agents/base/valid.agent.md: deeper than 2 levels"),
        "{lines:#?}"
    );
}

#[test]
fn catalog_resources_boundary() {
    let exact = Limits {
        catalog_resources: 6,
        ..Limits::PRODUCTION
    };
    assert_eq!(refusal_under(&exact), Vec::<String>::new());
    let past = Limits {
        catalog_resources: 5,
        ..Limits::PRODUCTION
    };
    assert_eq!(refusal_under(&past), ["catalog: more than 5 resources"]);
}
