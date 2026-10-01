//! Area principals, derived ownership and last-match delegation refusals.

use super::{
    area_packages::package_source,
    support::{MemoryTree, check_under},
};
use crate::{
    limits::Limits,
    source::{ReviewPath, ReviewRole, ReviewRule},
};

#[test]
fn area_owners_maintainers_validate() {
    let valid = package_source("package", "core");
    let check = |text: &str| {
        check_under(
            &MemoryTree::default().with("core/package.toml", text),
            &Limits::PRODUCTION,
        )
    };
    assert!(check(&valid).is_ok());
    for list in ["owners", "maintainers"] {
        for entries in [
            "\"bad name\"",
            "\"@org/team/extra\"",
            "\"-user\"",
            "\"user-\"",
            "\"user--name\"",
            "\"@org/-team\"",
            "\"@-org/team\"",
            "\"\"",
            "\"@org/\"",
            "\"user\", \"USER\"",
            "\"@org/team\", \"@ORG/TEAM\"",
            "\"user\", \"@USER\"",
        ] {
            let text = if list == "owners" {
                valid.replace(
                    "owners = [\"@synthetic/knowledge\"]",
                    &format!("owners = [{entries}]"),
                )
            } else {
                valid.replace(
                    "description =",
                    &format!("maintainers = [{entries}]\ndescription ="),
                )
            };
            let result = check(&text);
            assert!(result.is_err(), "{list} must refuse {entries}: {result:?}");
        }
    }
    for entries in ["", "\"reader\"", "\"@reader\"", "\"@synthetic/knowledge\""] {
        let text = valid.replace(
            "description =",
            &format!("maintainers = [{entries}]\ndescription ="),
        );
        assert!(check(&text).is_ok(), "{text}");
    }
    for (length, passes) in [(39, true), (40, false)] {
        let text = valid.replace("@synthetic/knowledge", &"a".repeat(length));
        assert_eq!(check(&text).is_ok(), passes, "username length {length}");
    }
    for (length, passes) in [(100, true), (101, false)] {
        let text = valid.replace(
            "@synthetic/knowledge",
            &format!("@org/{}", "a".repeat(length)),
        );
        assert_eq!(check(&text).is_ok(), passes, "team slug length {length}");
    }
    let empty = check(&valid.replace("owners = [\"@synthetic/knowledge\"]", "owners = []"));
    assert!(empty.is_err(), "owners must be nonempty");
    assert!(empty.unwrap_err().to_string().contains("nonempty"));
}

#[test]
fn resource_ownership_is_derived() {
    let tree = MemoryTree::valid().edit(
        "core/package.toml",
        "description =",
        "maintainers = [\"reader\"]\ndescription =",
    );
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    for resource in &catalog.resources {
        let ownership = catalog.ownership(resource).unwrap();
        assert_eq!(ownership.owners, ["@synthetic/knowledge"]);
        let core = resource.id.namespace.as_deref() == Some("core")
            || resource.id.to_string() == "package:core";
        assert_eq!(
            ownership.descriptor.path,
            if core {
                "core/package.toml"
            } else {
                "package.toml"
            }
        );
        assert_eq!(
            ownership.maintainers,
            if core { vec!["reader"] } else { vec![] }
        );
    }
    for (path, prefix, area) in [
        ("core/package.toml", "", "core/package.toml"),
        ("core/agents/valid.maestro.toml", "", "core/package.toml"),
        ("skills/valid-skill/SKILL.md", "  maestro.", "package.toml"),
    ] {
        for field in ["owner", "owners", "maintainers"] {
            let text = tree.text(path);
            let declared = if prefix.is_empty() {
                format!("{text}{field} = \"reader\"\n")
            } else {
                text.replace(
                    "  maestro.maturity:",
                    &format!("{prefix}{field}: reader\n  maestro.maturity:"),
                )
            };
            let result = check_under(&tree.clone().with(path, &declared), &Limits::PRODUCTION);
            assert!(result.is_err(), "resource-local {field} must refuse");
            let result = result.unwrap_err().to_string();
            assert!(
                result.contains("unknown key")
                    && result.contains(&format!("ownership is derived from {area}")),
                "{result}"
            );
        }
    }
    let missing = check_under(&tree.without("core/package.toml"), &Limits::PRODUCTION);
    assert!(missing.is_err(), "missing area ownership must refuse");
    let missing = missing.unwrap_err().to_string();
    assert!(
        missing.contains("missing area descriptor core/package.toml"),
        "{missing}"
    );
}

#[test]
fn groups_have_no_inherited_approval_authority() {
    let tree = MemoryTree::default()
        .with("package.toml", &package_source("package", "common"))
        .with(
            "capabilities/practice/review/package.toml",
            &package_source("package", "review").replace("@synthetic/knowledge", "team-owner"),
        )
        .with(
            "capabilities/practice/review/skills/valid-skill/SKILL.md",
            &MemoryTree::valid().text("skills/valid-skill/SKILL.md"),
        );
    let catalog = check_under(&tree, &Limits::PRODUCTION).unwrap();
    let skill = catalog
        .resources
        .iter()
        .find(|resource| resource.id.kind == "skill")
        .unwrap();
    let ownership = catalog.ownership(skill).unwrap();
    assert_eq!(ownership.owners, ["team-owner"]);
    assert_eq!(
        ownership.descriptor.path,
        "capabilities/practice/review/package.toml"
    );
    let missing = tree.without("capabilities/practice/review/package.toml");
    assert!(
        check_under(&missing, &Limits::PRODUCTION).is_err(),
        "common owners cannot substitute for team owners"
    );
}

#[test]
fn broad_codeowners_rule_cannot_override_descriptor() {
    let catalog = check_under(&MemoryTree::valid(), &Limits::PRODUCTION).unwrap();
    for area in catalog
        .resources
        .iter()
        .filter(|resource| resource.fields.contains_key("owners"))
    {
        let ownership = catalog.ownership(area).unwrap();
        let protected = [
            ReviewPath::Tree("exceptions".to_owned()),
            ReviewPath::Tree(".github/workflows".to_owned()),
            ReviewPath::Exact(".github/CODEOWNERS".to_owned()),
            ReviewPath::Exact("scripts/ownership-policy".to_owned()),
        ];
        let rules = ownership.review_rules(&protected);
        assert_eq!(rules[0].role, ReviewRole::Content);
        assert_eq!(rules[1].path, ReviewPath::Exact(area.path.clone()));
        assert_eq!(rules[1].role, ReviewRole::OwnersOnly);
        assert!(ownership.check_review_rules(&rules, &protected).is_ok());
        for file in [
            &area.path[..],
            "exceptions/standard.toml",
            ".github/workflows/check.yml",
            ".github/CODEOWNERS",
            "scripts/ownership-policy",
        ] {
            assert_eq!(
                rules
                    .iter()
                    .rev()
                    .find(|rule| rule.path.covers(file))
                    .unwrap()
                    .role,
                ReviewRole::OwnersOnly
            );
            let mut overridden = rules.clone();
            overridden.push(ReviewRule {
                path: ReviewPath::Exact(file.to_owned()),
                role: ReviewRole::Content,
            });
            assert!(
                ownership
                    .check_review_rules(&overridden, &protected)
                    .is_err(),
                "self-delegation to {file}"
            );
        }
        let mut subtree = rules.clone();
        subtree.push(ReviewRule {
            path: ReviewPath::Tree("exceptions/nested".to_owned()),
            role: ReviewRole::Content,
        });
        assert!(ownership.check_review_rules(&subtree, &protected).is_err());
        let mut overridden = rules.clone();
        overridden.push(ReviewRule {
            path: ReviewPath::Tree(String::new()),
            role: ReviewRole::Content,
        });
        assert!(
            ownership
                .check_review_rules(&overridden, &protected)
                .is_err(),
            "last broad rule cannot self-delegate"
        );
        assert!(ownership.check_review_rules(&[], &protected).is_err());
        let mut wrong_role = rules.clone();
        wrong_role[1].role = ReviewRole::Content;
        assert!(
            ownership
                .check_review_rules(&wrong_role, &protected)
                .is_err()
        );
        let mut reordered = rules.clone();
        reordered.swap(0, 1);
        assert!(
            ownership
                .check_review_rules(&reordered, &protected)
                .is_err()
        );
        let mut unrelated = rules.clone();
        unrelated.push(ReviewRule {
            path: ReviewPath::Tree("exceptions-other".to_owned()),
            role: ReviewRole::Content,
        });
        assert!(ownership.check_review_rules(&unrelated, &protected).is_ok());
    }
}

#[test]
fn registered_resource_fields_cannot_self_delegate() {
    use super::{registry::glossary, support::check_by};
    use crate::source::{Field, FieldType, builtin};
    let mut descriptor = glossary();
    descriptor.fields.extend([
        Field::optional("owner", FieldType::Text),
        Field::optional("owners", FieldType::TextList),
        Field::optional("maintainers", FieldType::TextList),
    ]);
    let mut registry = builtin().unwrap();
    registry.register(descriptor).unwrap();
    let metadata = package_source("package", "common");
    let source = format!(
        "term = \"evidence\"\n[metadata]{}",
        metadata.split_once("[metadata]").unwrap().1
    );
    let tree = MemoryTree::owned().with("glossaries/evidence.toml", &source);
    assert!(check_by(&tree, &registry, &Limits::PRODUCTION).is_ok());
    for declaration in [
        "owner = \"reader\"",
        "owners = [\"reader\"]",
        "maintainers = [\"reader\"]",
    ] {
        let edited = source.replace("[metadata]", &format!("{declaration}\n[metadata]"));
        let result = check_by(
            &tree.clone().with("glossaries/evidence.toml", &edited),
            &registry,
            &Limits::PRODUCTION,
        );
        assert!(
            result.is_err(),
            "registered local field cannot self-delegate: {declaration}: {result:?}"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("ownership is derived from package.toml")
        );
    }
}

#[test]
fn native_metadata_cannot_self_delegate() {
    let tree = MemoryTree::valid();
    let path = "skills/valid-skill/SKILL.md";
    for field in ["owner", "owners", "maintainers"] {
        let edited = tree.clone().edit(
            path,
            "  maestro.maturity:",
            &format!("  {field}: reader\n  maestro.maturity:"),
        );
        let result = check_under(&edited, &Limits::PRODUCTION);
        assert!(
            result.is_err(),
            "native ownership must not be silently ignored: {field}"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("ownership is derived from package.toml")
        );
    }
}

#[test]
fn resource_at_descriptor_path_is_not_an_ownership_record() {
    use super::{registry::glossary, support::check_by};
    use crate::source::Registry;
    let mut descriptor = glossary();
    descriptor.directory.clear();
    let mut registry = Registry::default();
    registry.register(descriptor).unwrap();
    let root = package_source("package", "common");
    let source = format!(
        "term = \"evidence\"\n[metadata]{}",
        root.split_once("[metadata]").unwrap().1
    );
    let tree = MemoryTree::default().with("package.toml", &source);
    let result = check_by(&tree, &registry, &Limits::PRODUCTION).unwrap_err();
    assert!(
        result
            .to_string()
            .contains("missing area descriptor package.toml")
    );
}

#[test]
fn review_paths_are_literal_and_catalog_relative() {
    let catalog = check_under(&MemoryTree::valid(), &Limits::PRODUCTION).unwrap();
    let area = catalog
        .resources
        .iter()
        .find(|resource| resource.id.to_string() == "package:core")
        .unwrap();
    let ownership = catalog.ownership(area).unwrap();
    for path in [
        "../outside",
        "/outside",
        "core/*",
        "./core",
        "core//nested",
        "core\\nested",
        "c:/file",
        "",
    ] {
        let protected = [ReviewPath::Exact(path.to_owned())];
        let rules = ownership.review_rules(&protected);
        assert!(
            ownership.check_review_rules(&rules, &protected).is_err(),
            "nonliteral or unsafe review path {path:?}"
        );
        if !path.is_empty() {
            let protected_tree = [ReviewPath::Tree(path.to_owned())];
            assert!(
                ownership
                    .check_review_rules(&ownership.review_rules(&protected_tree), &protected_tree)
                    .is_err(),
                "unsafe tree {path:?}"
            );
        }
        let mut extra = ownership.review_rules(&[]);
        extra.push(ReviewRule {
            path: ReviewPath::Exact(path.to_owned()),
            role: ReviewRole::OwnersOnly,
        });
        assert!(
            ownership.check_review_rules(&extra, &[]).is_err(),
            "unsafe extra rule {path:?}"
        );
    }
    assert!(
        ownership
            .check_review_rules(
                &ownership.review_rules(&[ReviewPath::Tree(String::new())]),
                &[ReviewPath::Tree(String::new())]
            )
            .is_ok()
    );
}

#[test]
fn last_match_reprotection_accepts() {
    let catalog = check_under(&MemoryTree::valid(), &Limits::PRODUCTION).unwrap();
    let area = catalog
        .resources
        .iter()
        .find(|resource| resource.id.to_string() == "package:core")
        .unwrap();
    let ownership = catalog.ownership(area).unwrap();
    let protected = [ReviewPath::Tree("exceptions".to_owned())];
    let mut rules = ownership.review_rules(&protected);
    assert!(ownership.check_review_rules(&rules, &protected).is_ok());
    let file = "exceptions/standard.toml";
    rules.push(ReviewRule {
        path: ReviewPath::Exact(file.to_owned()),
        role: ReviewRole::Content,
    });
    assert!(ownership.check_review_rules(&rules, &protected).is_err());
    rules.push(ReviewRule {
        path: ReviewPath::Exact(file.to_owned()),
        role: ReviewRole::OwnersOnly,
    });
    assert_eq!(
        rules
            .iter()
            .rev()
            .find(|rule| rule.path.covers(file))
            .unwrap()
            .role,
        ReviewRole::OwnersOnly
    );
    let checked = ownership.check_review_rules(&rules, &protected);
    assert!(
        checked.is_ok(),
        "final owner-only rule restores all protected paths: {checked:?}"
    );
    rules.push(ReviewRule {
        path: ReviewPath::Tree("exceptions/nested".to_owned()),
        role: ReviewRole::Content,
    });
    rules.push(ReviewRule {
        path: ReviewPath::Exact("exceptions/nested/one.toml".to_owned()),
        role: ReviewRole::OwnersOnly,
    });
    assert!(
        ownership.check_review_rules(&rules, &protected).is_err(),
        "one repaired file cannot protect a whole tree"
    );
    rules.push(ReviewRule {
        path: ReviewPath::Tree("exceptions/nested".to_owned()),
        role: ReviewRole::OwnersOnly,
    });
    assert!(ownership.check_review_rules(&rules, &protected).is_ok());
}

#[test]
fn legacy_owner_missing_maturity_points_to_area() {
    let tree = MemoryTree::valid();
    assert!(check_under(&tree, &Limits::PRODUCTION).is_ok());
    let path = "core/agents/valid.maestro.toml";
    let tree = tree.edit(path, "maturity = \"reviewed\"", "owner = \"reader\"");
    let refusal = check_under(&tree, &Limits::PRODUCTION).unwrap_err();
    let diagnostic = refusal
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.path == path && diagnostic.key == "owner")
        .unwrap();
    assert!(diagnostic.message.contains("unknown key"));
    assert!(
        diagnostic
            .message
            .contains("ownership is derived from core/package.toml"),
        "legacy owner must still point to its area: {diagnostic}"
    );
}
