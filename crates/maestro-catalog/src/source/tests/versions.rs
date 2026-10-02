//! C53a exact area pins and bounded runtime compatibility neighbours.

use super::{
    area_packages::package_source,
    support::{MemoryTree, check_by},
};
use crate::{
    limits::Limits,
    source::{Catalog, Refusal, builtin},
};

/// Check area declarations through the production source checker.
fn checked(tree: &MemoryTree) -> Result<Catalog, Refusal> {
    check_by(tree, &builtin().unwrap(), &Limits::PRODUCTION)
}

/// Optional version fields remain outside the metadata envelope.
fn area(kind: &str, name: &str, fields: &str, requires: &str) -> String {
    package_source(kind, name)
        .replace("[metadata]", &format!("{fields}\n[metadata]"))
        .replace("requires = []", &format!("requires = [{requires}]"))
}

#[test]
fn exact_package_pins_accept() {
    let version = env!("CARGO_PKG_VERSION");
    for requirement in [format!("={version}"), format!(">={version}, <999.0.0")] {
        let fields = format!(
            "runtime = \"{requirement}\"\n[dependency_pins]\n\"package:common\" = \"=1.2.3\""
        );
        let tree = MemoryTree::default()
            .with("package.toml", &package_source("package", "common"))
            .with(
                "core/package.toml",
                &area("package", "core", &fields, "\"package:common\""),
            )
            .with(
                "capabilities/practice/review/package.toml",
                &area("package", "review", &fields, "\"package:common\""),
            );
        let result = checked(&tree);
        assert!(result.is_ok(), "{result:#?}");
        let catalog = result.unwrap();
        let core = catalog
            .resources
            .iter()
            .find(|resource| resource.id.name == "core")
            .unwrap();
        assert_eq!(core.fields["runtime"].text(), Some(requirement.as_str()));
        assert!(core.fields.contains_key("dependency_pins"));
    }
    // Absence uses the snapshot's sole version, not a duplicated authored list.
    let tree = MemoryTree::default()
        .with("package.toml", &package_source("package", "common"))
        .with(
            "core/package.toml",
            &area("package", "core", "", "\"package:common\""),
        );
    assert!(checked(&tree).is_ok());
}

#[test]
fn conflicting_exact_pin_refuses() {
    for (first, second) in [("=1.2.3", "=2.0.0"), ("=2.0.0", "=1.2.3")] {
        let fields = |version| format!("[dependency_pins]\n\"package:common\" = \"{version}\"");
        let tree = MemoryTree::default()
            .with("package.toml", &package_source("package", "common"))
            .with(
                "core/package.toml",
                &area("package", "core", &fields(first), "\"package:common\""),
            )
            .with(
                "capabilities/practice/review/package.toml",
                &area("package", "review", &fields(second), "\"package:common\""),
            );
        let message = checked(&tree).unwrap_err().to_string();
        assert!(message.contains("package:common"), "{message}");
        assert!(
            message.contains("2.0.0") && message.contains("1.2.3"),
            "{message}"
        );
        assert!(message.contains("conflicting exact pin"), "{message}");
    }
}

#[test]
fn incompatible_runtime_refuses() {
    let upper_boundary = format!(">=0.0.0, <{}", env!("CARGO_PKG_VERSION"));
    for requirement in ["=999.0.0", ">=999.0.0, <1000.0.0", &upper_boundary] {
        let tree = MemoryTree::default().with(
            "package.toml",
            &area(
                "package",
                "common",
                &format!("runtime = \"{requirement}\""),
                "",
            ),
        );
        let message = checked(&tree).unwrap_err().to_string();
        for expected in [
            "package:common",
            requirement,
            env!("CARGO_PKG_VERSION"),
            "incompatible runtime",
        ] {
            assert!(message.contains(expected), "{message}");
        }
    }
}

#[test]
fn unsupported_runtime_requirement_refuses() {
    for requirement in [
        "^0.1.0",
        "~0.1.0",
        "*",
        "0.1.0",
        "=0.1",
        "=0.1.0-alpha",
        ">=0.1.0",
        "<1.0.0, >=0.1.0",
        ">=0.1.0, <=1.0.0",
        ">=0.1.0, <1.0.0, <2.0.0",
        "=0.1.0+build",
        "garbage",
    ] {
        let tree = MemoryTree::default().with(
            "package.toml",
            &area(
                "package",
                "common",
                &format!("runtime = \"{requirement}\""),
                "",
            ),
        );
        let message = checked(&tree).unwrap_err().to_string();
        assert!(
            message.contains("unsupported in Phase 1"),
            "{requirement}: {message}"
        );
    }
}

#[test]
fn invalid_dependency_pins_refuse() {
    for pin in [
        "1.2.3",
        "^1.2.3",
        "=1.2",
        "=1.2.3-alpha",
        ">=1.2.3, <2.0.0",
        "=1.2.3+build",
    ] {
        let tree = MemoryTree::default()
            .with("package.toml", &package_source("package", "common"))
            .with(
                "core/package.toml",
                &area(
                    "package",
                    "core",
                    &format!("[dependency_pins]\n\"package:common\" = \"{pin}\""),
                    "\"package:common\"",
                ),
            );
        let message = checked(&tree).unwrap_err().to_string();
        assert!(
            message.contains("exact") && message.contains("Phase 2"),
            "{pin}: {message}"
        );
    }
    for id in [
        "package:common",
        "preset:common",
        "common",
        "agent:core/worker",
    ] {
        let tree = MemoryTree::default()
            .with("package.toml", &package_source("package", "common"))
            .with(
                "core/package.toml",
                &area(
                    "package",
                    "core",
                    &format!("[dependency_pins]\n\"{id}\" = \"=1.2.3\""),
                    "",
                ),
            );
        let message = checked(&tree).unwrap_err().to_string();
        assert!(message.contains("required area"), "{message}");
    }
}

#[test]
fn language_and_standard_pins_use_package_rules() {
    for (kind, name, path) in [
        ("language", "rust", "languages/rust/package.toml"),
        ("standard", "security", "standards/security/package.toml"),
    ] {
        let tree = MemoryTree::default()
            .with("package.toml", &package_source("package", "common"))
            .with(
                path,
                &area(
                    kind,
                    name,
                    "[dependency_pins]\n\"package:common\" = \"=1.2.3\"",
                    "\"package:common\"",
                ),
            );
        assert!(checked(&tree).is_ok(), "{:?}", checked(&tree));
        let mismatch = tree.edit(path, "=1.2.3", "=2.0.0");
        assert!(
            checked(&mismatch)
                .unwrap_err()
                .to_string()
                .contains("conflicting exact pin")
        );
    }
}
