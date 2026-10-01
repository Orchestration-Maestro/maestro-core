//! D2's source limits at small injected values: each exact boundary passes
//! and one byte, level or resource past it is refused.

use super::support::{MemoryTree, VALID, check_under};
use crate::limits::Limits;
use crate::source::builtin;

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
        ["presets/knowledge-client.toml: deeper than 2 levels",]
    );
}

#[test]
fn yaml_frontmatter_depth_boundary() {
    let tree = MemoryTree::valid().edit(
        "core/agents/valid.agent.md",
        "tools: [\"view\"]",
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
            .any(|line| line.to_string() == "core/agents/valid.agent.md: deeper than 2 levels"),
        "{lines:#?}"
    );
}

#[test]
fn catalog_resources_boundary() {
    let exact = Limits {
        catalog_resources: 4,
        ..Limits::PRODUCTION
    };
    assert_eq!(refusal_under(&exact), Vec::<String>::new());
    let past = Limits {
        catalog_resources: 3,
        ..Limits::PRODUCTION
    };
    assert_eq!(refusal_under(&past), ["catalog: more than 3 resources"]);
}

#[test]
fn aggregate_walk_limit_refuses() {
    use super::area_support::{folder, registry};
    use crate::source::{Registry, walk::walk};
    let tree = MemoryTree::default()
        .with("glossaries/one.toml", "123")
        .with("core/glossaries/two.toml", "456");
    let exact = Limits {
        archive_entries: 5,
        archive_total_bytes: 6,
        ..Limits::PRODUCTION
    };
    assert!(walk(&tree, &registry(), &exact).is_ok());
    let entries = Limits {
        archive_entries: 4,
        ..exact
    };
    let result = walk(&tree, &registry(), &entries);
    assert!(result.is_err(), "entry bound must refuse");
    let error = result.unwrap_err();
    assert!(
        error.to_string().contains("more than 4 walk entries"),
        "{error}"
    );
    let bytes = Limits {
        archive_total_bytes: 5,
        ..exact
    };
    let result = walk(&tree, &registry(), &bytes);
    assert!(result.is_err(), "byte bound must refuse");
    let error = result.unwrap_err();
    assert!(
        error.to_string().contains("more than 5 source bytes"),
        "{error}"
    );
    let mut assets = Registry::default();
    assets.register(folder(&["assets/example.json"])).unwrap();
    let tree = MemoryTree::default()
        .with("skills/review/SKILL.md", "abc")
        .with("skills/review/assets/example.json", "def");
    assert!(walk(&tree, &assets, &exact).unwrap().diagnostics.is_empty());
    assert!(walk(&tree, &assets, &entries).is_err());
    assert!(walk(&tree, &assets, &bytes).is_err());
}

#[test]
fn aggregate_walk_counts_unchecked_legacy_support_and_depth() {
    use crate::source::walk::walk;
    let tree = MemoryTree::default().with("docs/nested/notes.txt", "abc");
    let bounds = Limits {
        archive_entries: 2,
        ..Limits::PRODUCTION
    };
    assert!(
        walk(&tree, &builtin().unwrap(), &bounds)
            .unwrap_err()
            .to_string()
            .contains("walk entries")
    );
    let depth = Limits {
        manifest_depth: 2,
        ..Limits::PRODUCTION
    };
    assert!(
        walk(&tree, &builtin().unwrap(), &depth).is_err(),
        "walk depth must refuse"
    );
    assert!(
        walk(&tree, &builtin().unwrap(), &depth)
            .unwrap_err()
            .to_string()
            .contains("walk depth")
    );
}

#[test]
fn scoped_resource_bound_is_independent_of_walk_entries() {
    use super::area_support::registry;
    use crate::source::walk::walk;
    let tree = MemoryTree::default()
        .with("glossaries/one.toml", "data")
        .with("core/glossaries/two.toml", "data");
    let exact = Limits {
        catalog_resources: 2,
        ..Limits::PRODUCTION
    };
    assert!(
        walk(&tree, &registry(), &exact)
            .unwrap()
            .diagnostics
            .is_empty()
    );
    let past = Limits {
        catalog_resources: 1,
        ..exact
    };
    assert!(
        walk(&tree, &registry(), &past).is_err(),
        "resource bound must refuse"
    );
    assert_eq!(
        walk(&tree, &registry(), &past).unwrap_err().to_string(),
        "catalog: more than 1 resources"
    );
}

#[test]
fn inert_asset_source_bytes_are_bounded_without_parsing() {
    use super::area_support::{discover, folder};
    use crate::source::Registry;
    let mut registry = Registry::default();
    registry.register(folder(&["assets/example.json"])).unwrap();
    let tree = MemoryTree::default()
        .with("skills/review/SKILL.md", "x")
        .with("skills/review/assets/example.json", "abc");
    let exact = Limits {
        source_file_bytes: 3,
        ..Limits::PRODUCTION
    };
    assert!(discover(&tree, &registry, &exact).diagnostics.is_empty());
    let past = Limits {
        source_file_bytes: 2,
        ..exact
    };
    let found = discover(&tree, &registry, &past);
    assert_eq!(found.diagnostics.len(), 1);
    assert_eq!(
        found.diagnostics[0].path,
        "skills/review/assets/example.json"
    );
    assert_eq!(found.diagnostics[0].message, "larger than 2 bytes");
}
