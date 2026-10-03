//! C35 rendering, exact drift comparison and protected last-match neighbours.

use super::{
    area_packages::package_source,
    support::{MemoryTree, check_under},
};
use crate::{limits::Limits, source::Catalog};

/// Five area records with distinct principals, including a duplicated delegate.
fn tree() -> MemoryTree {
    [
        ("package.toml", "package", "common"),
        ("core/package.toml", "package", "core"),
        (
            "capabilities/practice/review/package.toml",
            "package",
            "review",
        ),
        ("languages/rust/package.toml", "language", "rust"),
        ("standards/security/package.toml", "standard", "security"),
    ]
    .into_iter()
    .fold(MemoryTree::default(), |tree, (path, kind, name)| {
        let text = package_source(kind, name)
            .replace("@synthetic/knowledge", &format!("{name}-owner"))
            .replace(
                "description =",
                &format!("maintainers = [\"reader\", \"@{name}-owner\"]\ndescription ="),
            );
        super::language::area_content(tree, kind, name).with(path, &text)
    })
}

/// Check the real area schema, not manually assembled ownership records.
fn checked(tree: &MemoryTree) -> Catalog {
    check_under(tree, &Limits::PRODUCTION).unwrap()
}

/// Evaluate the emitted anchored exact/directory rules with last-match semantics.
fn reviewers<'a>(text: &'a str, path: &str) -> Vec<&'a str> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let pattern = words.next()?.strip_prefix('/')?;
            let covers = if pattern == "*" {
                true
            } else if let Some(parent) = pattern.strip_suffix('/') {
                path.strip_prefix(parent)
                    .is_some_and(|tail| tail.starts_with('/'))
            } else {
                pattern == path
            };
            covers.then(|| words.collect())
        })
        .next_back()
        .unwrap()
}

#[test]
fn fixture_codeowners_matches_golden() {
    let catalog = checked(&tree());
    let rendered = catalog.codeowners().unwrap();
    assert_eq!(
        rendered,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/catalog/codeowners/CODEOWNERS"
        ))
    );
    assert!(catalog.check_codeowners(&rendered).is_ok());
    // Resource iteration order cannot reorder governance rules.
    let mut reversed = catalog;
    reversed.resources.reverse();
    assert_eq!(reversed.codeowners().unwrap(), rendered);
}

#[test]
fn descriptor_owner_rule_wins_last() {
    let catalog = checked(&tree());
    let text = catalog.codeowners().unwrap();
    for (area, owner) in [
        ("", "common"),
        ("core/", "core"),
        ("capabilities/practice/review/", "review"),
        ("languages/rust/", "rust"),
        ("standards/security/", "security"),
    ] {
        assert_eq!(
            reviewers(&text, &format!("{area}skills/content/SKILL.md")),
            [format!("@{owner}-owner"), "@reader".to_owned()]
        );
        assert_eq!(
            reviewers(&text, &format!("{area}package.toml")),
            [format!("@{owner}-owner")]
        );
        let exception = reviewers(&text, &format!("{area}exceptions/nested/rule.toml"));
        assert!(!exception.contains(&"@reader"));
        assert!(exception.contains(&format!("@{owner}-owner").as_str()));
        if owner == "security" {
            assert_eq!(exception, ["@security-owner", "@common-owner"]);
        }
    }
    let protected = text
        .lines()
        .position(|line| line.starts_with("/package.toml "))
        .unwrap();
    assert!(
        text.lines()
            .skip(protected)
            .all(|line| !line.contains("@reader"))
    );
    // Appending a broad delegate is a refused neighbour, never a repaired rule.
    let overridden = format!("{text}/* @reader\n");
    assert!(catalog.check_codeowners(&overridden).is_err());
    assert_eq!(reviewers(&overridden, "core/package.toml"), ["@reader"]);
}

#[test]
fn root_owners_govern_generated_and_shared_files() {
    let text = checked(&tree()).codeowners().unwrap();
    for path in [
        ".github/CODEOWNERS",
        ".github/workflows/check.yml",
        "scripts/codeowners",
        "scripts/ownership-policy",
        "exceptions/standard.toml",
        "schemas/source.json",
        "fixtures/source.toml",
        "docs/ownership.md",
        "presets/client.toml",
        "marketplace/index.toml",
        "templates/package.toml",
    ] {
        assert_eq!(reviewers(&text, path), ["@common-owner"], "{path}");
    }
}

#[test]
fn codeowners_drift_refuses() {
    let catalog = checked(&tree());
    let text = catalog.codeowners().unwrap();
    assert!(catalog.check_codeowners(&text).is_ok());
    for (mutated, rule) in [
        (
            text.replace("/core/package.toml @core-owner\n", ""),
            "/core/package.toml",
        ),
        (
            text.replace(
                "/core/package.toml @core-owner",
                "/core/package.toml @reader",
            ),
            "/core/package.toml",
        ),
        (format!("{text}/obsolete/ @reader\n"), "/obsolete/"),
        (
            text.replace("/core/ @core-owner @reader", "/core/ @old-owner @reader"),
            "/core/",
        ),
        (
            text.replace(
                "/core/ @core-owner @reader\n",
                "/core/ @core-owner @reader\n/* @reader\n",
            ),
            "/*",
        ),
    ] {
        let refusal = catalog.check_codeowners(&mutated).unwrap_err();
        assert!(refusal.contains(rule), "{refusal}");
    }
    assert!(
        catalog.check_codeowners(text.trim_end()).is_err(),
        "exact bytes include final newline"
    );
}

#[test]
fn removed_area_rule_refuses() {
    let original = checked(&tree()).codeowners().unwrap();
    let catalog = checked(&tree().without("capabilities/practice/review/package.toml"));
    let current = catalog.codeowners().unwrap();
    assert!(catalog.check_codeowners(&current).is_ok());
    assert!(!current.contains("/capabilities/practice/review/"));
    let refusal = catalog.check_codeowners(&original).unwrap_err();
    assert!(
        refusal.contains("/capabilities/practice/review/"),
        "{refusal}"
    );
}

#[test]
fn source_check_validates_present_codeowners() {
    let tree = tree();
    let generated = checked(&tree).codeowners().unwrap();
    assert!(
        check_under(&tree, &Limits::PRODUCTION).is_ok(),
        "absent is allowed"
    );
    let present = tree.clone().with(".github/CODEOWNERS", &generated);
    assert!(check_under(&present, &Limits::PRODUCTION).is_ok());
    let drift = present.clone().edit(
        ".github/CODEOWNERS",
        "/core/package.toml @core-owner",
        "/core/package.toml @reader",
    );
    let refusal = check_under(&drift, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(refusal.contains("/core/package.toml"), "{refusal}");
    assert!(
        check_under(
            &present.clone().with_link(".github/CODEOWNERS"),
            &Limits::PRODUCTION
        )
        .is_err()
    );
    assert!(
        check_under(
            &tree.clone().with(".github/CODEOWNERS/file", "data"),
            &Limits::PRODUCTION
        )
        .is_err()
    );
    assert!(
        check_under(
            &present.clone().with(".github/unrelated", "data"),
            &Limits::PRODUCTION
        )
        .is_err()
    );
    let limits = Limits {
        source_file_bytes: u64::try_from(generated.len()).unwrap() - 1,
        ..Limits::PRODUCTION
    };
    let refusal = check_under(&present, &limits).unwrap_err().to_string();
    assert!(
        refusal.contains("CODEOWNERS") && refusal.contains("larger than"),
        "{refusal}"
    );
}

#[test]
fn codeowners_unsafe_or_ambiguous_area_refuses() {
    let mut catalog = checked(&tree());
    let area = catalog
        .resources
        .iter_mut()
        .find(|resource| resource.id.name == "review")
        .unwrap();
    area.path = "capabilities/practice/rev#iew/package.toml".to_owned();
    let refusal = catalog.codeowners().unwrap_err();
    assert!(refusal.contains("unsafe CODEOWNERS rule path"), "{refusal}");
    let mut catalog = checked(&tree());
    let duplicate = catalog
        .resources
        .iter()
        .find(|resource| resource.id.name == "review")
        .unwrap()
        .clone();
    catalog.resources.push(duplicate);
    assert!(
        catalog
            .codeowners()
            .unwrap_err()
            .contains("ambiguous ownership")
    );
}

#[test]
fn codeowners_render_refuses_oversized_without_truncating_reviewers() {
    for count in [10, 20_000] {
        let owners = (0..count)
            .map(|index| format!("\"owner-{index:05}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let source =
            package_source("package", "common").replace("\"@synthetic/knowledge\"", &owners);
        let catalog = checked(&MemoryTree::default().with("package.toml", &source));
        let rendered = catalog.codeowners();
        if count == 20_000 {
            assert!(rendered.is_err(), "oversized CODEOWNERS must refuse");
            let refusal = rendered.unwrap_err();
            assert!(refusal.contains("CODEOWNERS"), "{refusal}");
            assert!(refusal.contains("larger than 1048576 bytes"), "{refusal}");
        } else {
            let rendered = rendered.unwrap();
            let expected: Vec<_> = (0..count)
                .map(|index| format!("@owner-{index:05}"))
                .collect();
            assert_eq!(reviewers(&rendered, "package.toml"), expected);
            assert!(catalog.check_codeowners(&rendered).is_ok());
        }
    }
}

#[test]
fn codeowners_size_ceiling_is_inclusive() {
    let limit = 64;
    assert!(super::super::ownership::within(&"x".repeat(64), limit).is_ok());
    let refusal = super::super::ownership::within(&"x".repeat(65), limit).unwrap_err();
    assert!(refusal.contains("larger than 64 bytes"), "{refusal}");
}

#[test]
fn codeowners_requires_root_ownership() {
    let tree = MemoryTree::default().with("core/package.toml", &package_source("package", "core"));
    assert!(
        checked(&tree)
            .codeowners()
            .unwrap_err()
            .contains("root area ownership")
    );
}

#[test]
fn codeowners_does_not_treat_content_or_presets_as_an_area() {
    let catalog = checked(&MemoryTree::valid());
    let text = catalog.codeowners().unwrap();
    for descriptor in ["/package.toml ", "/core/package.toml "] {
        assert_eq!(
            text.lines()
                .filter(|line| line.starts_with(descriptor))
                .count(),
            1,
            "{text}"
        );
    }
    assert!(!text.contains("/core/agents/valid.agent.md"));
}
