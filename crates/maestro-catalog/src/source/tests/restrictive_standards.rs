//! Restrictive standards and scoped, centrally declared exceptions.

use super::{area_packages::package_source, support::MemoryTree};
use crate::settings::resolve::resolve_with_standards;
use crate::{
    limits::Limits,
    source::{Catalog, Known, Refusal, Scope, builtin, builtin_hooks, check, frozen_rows},
};
use maestro_settings::{BUILT_IN, Registry, SettingClass, SettingDescriptor, SettingKind};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Synthetic controls, never a catalog-private key registry.
fn settings() -> Registry {
    let mut descriptors = BUILT_IN.to_vec();
    for (key, kind, default, class, standard_only) in [
        (
            "test.ceiling",
            SettingKind::Integer {
                min: 1,
                max: 100,
                off: false,
                power_of_two: false,
            },
            "100",
            SettingClass::Bounded,
            false,
        ),
        (
            "test.permission",
            SettingKind::Flag,
            "true",
            SettingClass::Bounded,
            false,
        ),
        (
            "test.check",
            SettingKind::Flag,
            "false",
            SettingClass::Additive,
            false,
        ),
        (
            "test.permissions",
            SettingKind::ChoiceList {
                values: Cow::Owned(vec![Cow::Borrowed("read"), Cow::Borrowed("write")]),
            },
            "read,write",
            SettingClass::Bounded,
            false,
        ),
        (
            "test.checks",
            SettingKind::ChoiceList {
                values: Cow::Owned(vec![Cow::Borrowed("lint"), Cow::Borrowed("scan")]),
            },
            "lint",
            SettingClass::Additive,
            false,
        ),
        (
            "secret_scan.enabled",
            SettingKind::Flag,
            "false",
            SettingClass::Additive,
            false,
        ),
        (
            "secret_scan.allow_list",
            SettingKind::ChoiceList {
                values: Cow::Owned(vec![Cow::Borrowed("synthetic")]),
            },
            "",
            SettingClass::Bounded,
            true,
        ),
    ] {
        descriptors.push(SettingDescriptor {
            key: Cow::Borrowed(key),
            kind,
            default: Cow::Borrowed(default),
            description: Cow::Borrowed("Synthetic control."),
            class,
            standard_only,
        });
    }
    Registry::new(&descriptors).unwrap()
}

/// Fixed clock; the catalog never reads system time.
fn checked(tree: &MemoryTree) -> Result<Catalog, Refusal> {
    check(
        tree,
        &builtin().unwrap(),
        &Limits::PRODUCTION,
        Known {
            rows: &frozen_rows(),
            settings: &settings(),
            today: 20_727,
        },
    )
}

/// A standard using the existing area envelope and rule inventory.
fn tree(controls: &str) -> MemoryTree {
    let standard = package_source("package", "security")
        .replace("kind = \"package\"", "kind = \"standard\"")
        .replace(
            "[metadata]",
            &format!("rules = [\"SEC-001\"]\n{controls}\n[metadata]"),
        );
    MemoryTree::valid().with("standards/security/package.toml", &standard)
}

/// Settings must precede the metadata table.
fn consumer(tree: MemoryTree, path: &str, controls: &str) -> MemoryTree {
    let source = tree.text(path);
    if Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
    {
        let parsed: toml::Value = toml::from_str(controls).unwrap();
        let settings = serde_json::to_string(&parsed["settings"]).unwrap();
        tree.with(
            path,
            &source.replacen("---\n", &format!("---\nsettings: {settings}\n"), 1),
        )
    } else {
        tree.with(
            path,
            &source.replace("[metadata]", &format!("{controls}\n[metadata]")),
        )
    }
}

/// Exact central record: expiry is exclusive, so today equal to expiry refuses.
fn exception() -> &'static str {
    "name = \"temporary\"\nrule = \"SEC-001\"\nscopes = [\"package:core\"]\n\
     rationale = \"Synthetic migration\"\nexpiry = \"2026-10-02\"\n\
     evidence = \"standard-check:security/approval\"\n[metadata]\n\
     schema = \"maestro-source/2\"\nmaturity = \"reviewed\"\n\
     rows = [\"chat.M036 objects\"]\nworkflows = [\"ctm-question\"]\n"
}

#[test]
fn invalid_defaults_preserve_unreadable_diagnostics() {
    let tree = MemoryTree::valid()
        .with(
            "settings/defaults.toml",
            "schema = \"maestro-preferences/1\"\nlanguage = \"en-US-extra\"\n",
        )
        .with_unreadable("core/agents/valid.maestro.toml");
    let refusal = checked(&tree).unwrap_err();
    let diagnostics = refusal.to_string();
    assert!(diagnostics.contains("settings/defaults.toml: language:"));
    assert!(diagnostics.contains("core/agents/valid.maestro.toml: cannot read"));
    assert!(refusal.unreadable());
}

#[test]
fn manifest_defaults_reach_production_standards() {
    let base = consumer(
        tree("[settings]\n\"test.check\" = false"),
        "core/package.toml",
        "[settings]\n\"test.check\" = false",
    );
    assert!(checked(&base).is_ok(), "{:?}", checked(&base));
    let with_defaults = base.with(
        "settings/defaults.toml",
        "schema = \"maestro-preferences/1\"\n\"test.check\" = true\n",
    );
    assert!(
        checked(&with_defaults)
            .unwrap_err()
            .to_string()
            .contains("widens standard")
    );
}

#[test]
fn standard_constraints_only_narrow() {
    let base = tree(
        "[settings]\n\"test.ceiling\" = 50\n\"test.permission\" = false\n\
         \"test.check\" = true\n\"test.permissions\" = [\"read\"]\n\
         \"test.checks\" = [\"lint\"]",
    )
    .language("rust");
    for path in [
        "languages/rust/package.toml",
        "core/package.toml",
        "core/instructions/valid.instructions.md",
    ] {
        let valid = consumer(
            base.clone(),
            path,
            "[settings]\n\"test.ceiling\" = 40\n\"test.permission\" = false\n\
             \"test.check\" = true\n\"test.permissions\" = [\"read\"]\n\
             \"test.checks\" = [\"scan\"]",
        );
        assert!(checked(&valid).is_ok(), "{:?}", checked(&valid));
        for controls in [
            "\"test.ceiling\" = 60",
            "\"test.permission\" = true",
            "\"test.check\" = false",
            "\"test.permissions\" = [\"write\"]",
        ] {
            let invalid = consumer(base.clone(), path, &format!("[settings]\n{controls}"));
            assert!(
                checked(&invalid)
                    .unwrap_err()
                    .to_string()
                    .contains("widens standard")
            );
        }
    }
    let conflict = tree("[settings]\n\"tone\" = \"brief\"");
    assert!(
        checked(&conflict)
            .unwrap_err()
            .to_string()
            .contains("restrictive override class")
    );
}

#[test]
fn local_expired_or_wider_exception_refuses() {
    let base = consumer(
        tree(""),
        "core/package.toml",
        "exceptions = [\"standard-exception:common/temporary\"]",
    );
    let valid = base.clone().with("exceptions/temporary.toml", exception());
    assert!(checked(&valid).is_ok(), "{:?}", checked(&valid));
    for path in [
        "core/exceptions/temporary.toml",
        "languages/demo/exceptions/temporary.toml",
    ] {
        assert!(checked(&base.clone().with(path, exception())).is_err());
    }
    for (from, to, expected) in [
        ("2026-10-02", "2026-10-01", "expired"),
        ("2026-10-02", "2026-09-30", "expired"),
        ("package:core", "package:common", "outside approved scopes"),
        ("package:core", "package:*", "exact qualified ID"),
        ("SEC-001", "SEC-999", "unknown inventory rule"),
        (
            "standard-check:security/approval",
            "https://example.test/approval",
            "typed evidence reference",
        ),
    ] {
        let invalid = valid
            .clone()
            .with("exceptions/temporary.toml", &exception().replace(from, to));
        assert!(
            checked(&invalid)
                .unwrap_err()
                .to_string()
                .contains(expected),
            "{expected}"
        );
    }
    for date in [
        "2026-10-002",
        "2026/10/02",
        "2026-10/02",
        "0000-10-02",
        "2026-02-30",
        "2026-13-01",
        "2026-1-01",
        "2026-10-02T00:00:00Z",
    ] {
        let invalid = valid.clone().with(
            "exceptions/temporary.toml",
            &exception().replace("2026-10-02", date),
        );
        assert!(
            checked(&invalid)
                .unwrap_err()
                .to_string()
                .contains("calendar date")
        );
    }
    let leap = valid.with(
        "exceptions/temporary.toml",
        &exception().replace("2026-10-02", "2028-02-29"),
    );
    assert!(checked(&leap).is_ok());
}

#[test]
fn nonnegotiable_exception_refuses() {
    let valid = tree("").with("standards/security/exceptions/temporary.toml", exception());
    assert!(checked(&valid).is_ok(), "{:?}", checked(&valid));
    let invalid = tree("non_negotiable = [\"SEC-001\"]")
        .with("standards/security/exceptions/temporary.toml", exception());
    assert!(
        checked(&invalid)
            .unwrap_err()
            .to_string()
            .contains("non-negotiable")
    );
    assert!(
        checked(&tree("non_negotiable = [\"SEC-999\"]"))
            .unwrap_err()
            .to_string()
            .contains("not in rule inventory")
    );
}

#[test]
fn secret_scan_weakening_refuses() {
    let base = tree(
        "[settings]\n\"secret_scan.enabled\" = true\n\"secret_scan.allow_list\" = [\"synthetic\"]",
    );
    assert!(checked(&base).is_ok(), "{:?}", checked(&base));
    let weaker = consumer(
        base.clone(),
        "core/package.toml",
        "[settings]\n\"secret_scan.enabled\" = false",
    );
    assert!(
        checked(&weaker)
            .unwrap_err()
            .to_string()
            .contains("widens standard")
    );
    for base in [base, tree("")] {
        for values in ["[]", "[\"synthetic\"]"] {
            let local = consumer(
                base.clone(),
                "core/package.toml",
                &format!("[settings]\n\"secret_scan.allow_list\" = {values}"),
            );
            assert!(
                checked(&local)
                    .unwrap_err()
                    .to_string()
                    .contains("standard-only")
            );
        }
    }
}

#[test]
fn standard_settings_and_exception_shapes_remain_strict() {
    let valid = tree("");
    for controls in [
        "settings = true",
        "[settings]\nunknown = true",
        "[settings]\n\"test.ceiling\" = \"wrong\"",
        "[settings]\n\"test.permissions\" = [\"unknown\"]",
    ] {
        assert!(checked(&consumer(valid.clone(), "core/package.toml", controls)).is_err());
    }
    let local = consumer(
        valid.clone(),
        "core/package.toml",
        "exceptions = [\"standard-exception:common/missing\"]",
    );
    assert!(
        checked(&local)
            .unwrap_err()
            .to_string()
            .contains("unknown central exception")
    );
    for (from, to, message) in [
        (
            "scopes = [\"package:core\"]",
            "scopes = []",
            "exact qualified ID",
        ),
        ("package:core", "package:missing", "unknown scope"),
        ("2026-10-02", "2027-02-29", "calendar date"),
        ("2026-10-02", "2100-02-29", "calendar date"),
        ("2026-10-02", "2026-00-01", "calendar date"),
        ("2026-10-02", "2026-01-00", "calendar date"),
        (
            "rationale = \"Synthetic migration\"",
            "rationale = \"\"",
            "nonempty string",
        ),
        (
            "name = \"temporary\"",
            "name = \"temporary\"\napproved = true",
            "unknown key",
        ),
    ] {
        let invalid = valid
            .clone()
            .with("exceptions/temporary.toml", &exception().replace(from, to));
        assert!(
            checked(&invalid).unwrap_err().to_string().contains(message),
            "{message}"
        );
    }
    let foreign = valid
        .clone()
        .with(
            "standards/quality/package.toml",
            &package_source("standard", "quality"),
        )
        .with("standards/quality/exceptions/temporary.toml", exception());
    assert!(
        checked(&foreign)
            .unwrap_err()
            .to_string()
            .contains("different standard")
    );
    let mut registry = builtin().unwrap();
    let mut unapproved = registry
        .kind("standard-exception")
        .unwrap()
        .descriptor
        .clone();
    unapproved.kind = "other-exception".to_owned();
    unapproved.directory = "other-governance".to_owned();
    assert!(registry.register(unapproved).is_err());
    let names = settings()
        .descriptors()
        .map(|descriptor| descriptor.key.to_string())
        .collect::<BTreeSet<_>>();
    let rows = frozen_rows();
    assert!(
        check(
            &tree("[settings]\n\"test.ceiling\" = 10"),
            &builtin().unwrap(),
            &Limits::PRODUCTION,
            Known {
                rows: &rows,
                settings: &names,
                today: 20_727
            }
        )
        .unwrap_err()
        .to_string()
        .contains("typed descriptors")
    );
}

#[test]
fn standard_only_preferences_and_unordered_conflicts_refuse() {
    let registry = settings();
    let flags = [maestro_settings::Flag {
        key: "secret_scan.allow_list".to_owned(),
        value: maestro_settings::Value::List(Vec::new()),
    }];
    let resolved =
        maestro_settings::resolve(&registry, &maestro_settings::Layers::default(), &flags);
    assert!(
        resolve_with_standards(&registry, &resolved, &BTreeMap::new())
            .get("secret_scan.allow_list")
            .unwrap()
            .is_err()
    );
    let base = tree("[settings]\nmodel_profile = \"fast\"");
    let second = package_source("standard", "quality").replace(
        "[metadata]",
        "[settings]\nmodel_profile = \"deep\"\n[metadata]",
    );
    assert!(
        checked(&base.with("standards/quality/package.toml", &second))
            .unwrap_err()
            .to_string()
            .contains("conflicting standard constraints")
    );
}

#[test]
fn a_descriptor_cannot_admit_local_exceptions() {
    let mut registry = builtin_hooks();
    for registration in builtin().unwrap().registrations() {
        let mut descriptor = registration.descriptor.clone();
        if descriptor.kind == "standard-exception" {
            descriptor.scopes = vec![Scope::Core];
        }
        registry.register(descriptor).unwrap();
    }
    let rows = frozen_rows();
    let settings = settings();
    let tree = tree("").with("core/exceptions/temporary.toml", exception());
    assert!(
        check(
            &tree,
            &registry,
            &Limits::PRODUCTION,
            Known {
                rows: &rows,
                settings: &settings,
                today: 20_727
            }
        )
        .unwrap_err()
        .to_string()
        .contains("local exception refuses")
    );
}
