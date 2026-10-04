//! Root language contracts use shared areas and honest inert quality declarations.

use super::{
    area_packages::package_source,
    support::{MemoryTree, check_under},
};
use crate::{limits::Limits, source::Value};

/// Authored synthetic inputs, independent of the validator.
macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/catalog/languages/",
            $name
        ))
    };
}

/// Complete synthetic language content for existing area/closure fixture builders.
pub(super) fn declarations(tree: MemoryTree, name: &str) -> MemoryTree {
    tree.with(
        "standards/quality/package.toml",
        &package_source("standard", "quality"),
    )
    .with(
        "standards/quality/profiles/quality/baseline.toml",
        fixture!("baseline.toml"),
    )
    .with(
        &format!("languages/{name}/profiles/quality/default.toml"),
        &fixture!("manager-choices.toml")
            .replace("subject = \"synthetic\"", &format!("subject = \"{name}\"")),
    )
    .with(
        &format!("languages/{name}/instructions/rules.instructions.md"),
        fixture!("rules.instructions.md"),
    )
    .with(
        &format!("languages/{name}/instructions/rules.maestro.toml"),
        fixture!("rules.maestro.toml"),
    )
    .with(
        &format!("languages/{name}/bootstrap/starter.toml"),
        fixture!("starter.toml"),
    )
}

/// Add only the declarations a language-area fixture requires.
pub(super) fn area_content(tree: MemoryTree, kind: &str, name: &str) -> MemoryTree {
    if kind == "language" {
        declarations(tree, name)
    } else {
        tree
    }
}

/// The complete root with a standard baseline and unresolved gate implementations.
fn tree() -> MemoryTree {
    declarations(MemoryTree::owned(), "synthetic")
        .with("languages/synthetic/package.toml", fixture!("package.toml"))
}

/// A changed file must refuse for the named guard, not a missing neighbour.
fn refuses(path: &str, text: &str, message: &str) {
    let refusal = check_under(&tree().with(path, text), &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(refusal.contains(message), "{refusal}");
}

#[test]
fn language_has_one_id_and_manager_default() {
    let catalog = check_under(&tree(), &Limits::PRODUCTION).unwrap();
    let ids: Vec<_> = catalog
        .resources
        .iter()
        .filter(|resource| resource.id.name == "synthetic")
        .map(|resource| resource.id.to_string())
        .collect();
    assert_eq!(ids, ["language:synthetic"]);
    let profile = catalog
        .resources
        .iter()
        .find(|resource| resource.id.to_string() == "quality-profile:synthetic/default")
        .unwrap();
    let Value::Table(managers) = &profile.fields["managers"] else {
        panic!("managers table")
    };
    let Value::Table(choice) = &managers["environment"] else {
        panic!("choice table")
    };
    assert_eq!(choice["default"].text(), Some("uv"));
    assert_eq!(choice["alternatives"].texts().unwrap(), ["poetry"]);
    assert_eq!(
        profile.fields["gates"].texts().unwrap(),
        [
            "format", "lint", "types", "security", "secrets", "mutation", "property", "coverage"
        ]
    );
}

#[test]
fn duplicate_manager_default_refuses() {
    assert!(check_under(&tree(), &Limits::PRODUCTION).is_ok());
    refuses(
        "languages/synthetic/profiles/quality/default.toml",
        &fixture!("manager-choices.toml")
            .replace("default = \"uv\"", "default = \"uv\"\ndefault = \"poetry\""),
        "duplicate key",
    );
    refuses(
        "languages/synthetic/profiles/quality/default.toml",
        &fixture!("manager-choices.toml")
            .replace("alternatives = [\"poetry\"]", "alternatives = [\"uv\"]"),
        "repeats default or alternative",
    );
}

#[test]
fn language_product_path_refuses() {
    let neighbour = tree().edit(
        "languages/synthetic/package.toml",
        "technology = \"uv\"",
        "technology = \"qdrant\"",
    );
    assert!(check_under(&neighbour, &Limits::PRODUCTION).is_ok());
    let invalid = tree().with(
        "languages/synthetic/profiles/quality/qdrant.toml",
        &fixture!("manager-choices.toml").replace("name = \"default\"", "name = \"qdrant\""),
    );
    let refusal = check_under(&invalid, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(refusal.contains("product"), "{refusal}");
}

#[test]
fn language_cannot_weaken_standard() {
    assert!(check_under(&tree(), &Limits::PRODUCTION).is_ok());
    refuses(
        "languages/synthetic/profiles/quality/default.toml",
        &fixture!("manager-choices.toml").replace("coverage = 90.0", "coverage = 79.0"),
        "weakens floor coverage",
    );
    refuses(
        "languages/synthetic/profiles/quality/default.toml",
        &fixture!("manager-choices.toml").replace("warnings = 2", "warnings = 6"),
        "weakens ceiling warnings",
    );
}

#[test]
fn language_required_references_are_typed_owner_local_and_present() {
    for (from, to, message) in [
        (
            "quality-profile:synthetic/default",
            "not-an-id",
            "owner-local quality-profile",
        ),
        (
            "quality-profile:synthetic/default",
            "skill:synthetic/default",
            "owner-local quality-profile",
        ),
        (
            "quality-profile:synthetic/default",
            "quality-profile:quality/baseline",
            "owner-local quality-profile",
        ),
        (
            "quality-profile:synthetic/default",
            "quality-profile:synthetic/missing",
            "which does not exist",
        ),
        (
            "instructions:synthetic/rules",
            "instructions:core/rules",
            "owner-local instructions",
        ),
        (
            "bootstrap-inventory:synthetic/starter",
            "skill:synthetic/starter",
            "owner-local bootstrap-inventory",
        ),
        (
            "instructions = [\"instructions:synthetic/rules\"]",
            "instructions = []",
            "nonempty list",
        ),
        (
            "starter = [\"bootstrap-inventory:synthetic/starter\"]",
            "starter = []",
            "nonempty list",
        ),
    ] {
        refuses(
            "languages/synthetic/package.toml",
            &fixture!("package.toml").replace(from, to),
            message,
        );
    }
    for field in ["technology", "quality_profile", "instructions", "starter"] {
        let missing = fixture!("package.toml")
            .lines()
            .filter(|line| !line.starts_with(&format!("{field} =")))
            .collect::<Vec<_>>()
            .join("\n");
        refuses(
            "languages/synthetic/package.toml",
            &missing,
            &format!("{field}: missing"),
        );
    }
}

#[test]
fn language_quality_requires_every_category_and_standard_baseline() {
    let profile = fixture!("manager-choices.toml");
    for gate in [
        "format", "lint", "types", "security", "secrets", "mutation", "property", "coverage",
    ] {
        refuses(
            "languages/synthetic/profiles/quality/default.toml",
            &profile
                .replace(&format!("[bindings.{gate}]\nstate = \"unresolved\"\n"), "")
                .replace(&format!("  \"{gate}\",\n"), ""),
            &format!("missing language gate category {gate}"),
        );
    }
    refuses(
        "languages/synthetic/profiles/quality/default.toml",
        &profile.replace("baseline = \"quality-profile:quality/baseline\"\n", ""),
        "standard-owned baseline",
    );
    let nonstandard = tree()
        .with("profiles/quality/baseline.toml", fixture!("baseline.toml"))
        .with(
            "languages/synthetic/profiles/quality/default.toml",
            &profile.replace(
                "quality-profile:quality/baseline",
                "quality-profile:common/baseline",
            ),
        );
    let refusal = check_under(&nonstandard, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(refusal.contains("standard-owned baseline"), "{refusal}");
}

#[test]
fn language_manager_choices_are_strict_and_single_default() {
    let profile = fixture!("manager-choices.toml");
    let empty_alternatives = tree().edit(
        "languages/synthetic/profiles/quality/default.toml",
        "alternatives = [\"poetry\"]",
        "alternatives = []",
    );
    assert!(check_under(&empty_alternatives, &Limits::PRODUCTION).is_ok());
    for (from, to, message) in [
        ("default = \"uv\"", "default = \" \"", "nonempty manager"),
        (
            "alternatives = [\"poetry\"]",
            "alternatives = [\"poetry\", \"poetry\"]",
            "repeats default or alternative",
        ),
        (
            "alternatives = [\"poetry\"]",
            "alternatives = [\"\"]",
            "nonempty manager",
        ),
        ("alternatives = [\"poetry\"]\n", "", "missing field"),
        ("default = \"uv\"\n", "", "missing field"),
        (
            "[managers.environment]",
            "[managers.qdrant]",
            "functional manager choice name",
        ),
        (
            "default = \"uv\"",
            "default = \"uv\"\nweaker = true",
            "unknown field",
        ),
        (
            "[managers.environment]\ndefault = \"uv\"\nalternatives = [\"poetry\"]\n",
            "[managers]\n",
            "nonempty managers table",
        ),
        (
            "[managers.environment]",
            "[managers.\" \"]",
            "nonempty manager",
        ),
    ] {
        refuses(
            "languages/synthetic/profiles/quality/default.toml",
            &profile.replace(from, to),
            message,
        );
    }
    let positional_managers = profile.replace(
        "[managers.environment]\ndefault = \"uv\"\nalternatives = [\"poetry\"]\n",
        "",
    );
    refuses(
        "languages/synthetic/profiles/quality/default.toml",
        &format!("managers = [{{default = \"uv\", alternatives = []}}]\n{positional_managers}"),
        "managers must be a table",
    );
    let absent = profile.replace(
        "[managers.environment]\ndefault = \"uv\"\nalternatives = [\"poetry\"]\n",
        "",
    );
    refuses(
        "languages/synthetic/profiles/quality/default.toml",
        &absent,
        "language profile requires managers",
    );
    let positional = profile.replace(
        "[managers.environment]\ndefault = \"uv\"\nalternatives = [\"poetry\"]\n",
        "[managers]\nenvironment = [\"uv\", \"poetry\"]\n",
    );
    refuses(
        "languages/synthetic/profiles/quality/default.toml",
        &positional,
        "must be a table",
    );
}
