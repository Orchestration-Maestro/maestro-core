//! Quality-profile declarations, narrowing refusals and honest unresolved bindings.

use super::support::{MemoryTree, check_under};
use crate::{
    limits::Limits,
    source::{ResourceId, Value, builtin},
};

/// Read authored fixtures independently of the validator.
macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/catalog/quality/",
            $name,
            ".toml"
        ))
    };
}

/// A common baseline and a stricter owner-local technology declaration.
fn tree() -> MemoryTree {
    MemoryTree::valid()
        .with("profiles/quality/baseline.toml", fixture!("baseline"))
        .with("profiles/quality/technology.toml", fixture!("technology"))
}

/// Check one changed profile and retain the specific diagnostic.
fn refuses(text: &str, message: &str) {
    let refusal = check_under(
        &tree().with("profiles/quality/technology.toml", text),
        &Limits::PRODUCTION,
    )
    .unwrap_err()
    .to_string();
    assert!(refusal.contains(message), "{refusal}");
}

#[test]
fn quality_profile_is_not_session_profile() {
    let registry = builtin().unwrap();
    let quality = registry.kind("quality-profile").unwrap();
    assert_eq!(quality.descriptor.directory, "profiles/quality");
    let catalog = check_under(&tree(), &Limits::PRODUCTION).unwrap();
    let id = ResourceId::parse("quality-profile:common/technology").unwrap();
    assert!(catalog.resources.iter().any(|resource| resource.id == id));
    assert!(
        !catalog
            .resources
            .iter()
            .any(|resource| resource.id.kind == "session-profile")
    );
    refuses(fixture!("session-field"), "model_profile: unknown key");
    for key in ["provider", "role", "settings", "session_profile"] {
        refuses(
            &fixture!("technology").replace("subject =", &format!("{key} = \"deep\"\nsubject =")),
            "unknown key",
        );
    }
    let wrong = fixture!("technology").replace(
        "quality-profile:common/baseline",
        "session-profile:common/baseline",
    );
    refuses(&wrong, "baseline must reference a quality-profile");
    let session_path = tree()
        .without("profiles/quality/technology.toml")
        .with("profiles/models/technology.toml", fixture!("technology"));
    assert!(check_under(&session_path, &Limits::PRODUCTION).is_err());
}

#[test]
fn quality_thresholds_only_narrow() {
    assert!(check_under(&tree(), &Limits::PRODUCTION).is_ok());
    refuses(fixture!("weakened-floor"), "weakens floor coverage");
    refuses(fixture!("weakened-ceiling"), "weakens ceiling warnings");
    for (from, message) in [
        ("coverage = 90.0\n", "removed floor coverage"),
        ("warnings = 2\n", "removed ceiling warnings"),
    ] {
        refuses(&fixture!("technology").replace(from, ""), message);
    }
    refuses(
        &fixture!("technology").replace("warnings = 2", "warnings = -1"),
        "nonnegative finite",
    );
    for value in ["nan", "inf", "-inf"] {
        refuses(
            &fixture!("technology").replace("coverage = 90.0", &format!("coverage = {value}")),
            "finite number",
        );
    }
    refuses(
        &fixture!("technology").replace("coverage = 90.0", "coverage = -0.5"),
        "nonnegative finite",
    );
    refuses(
        &fixture!("technology").replace("warnings = 2", "\" \" = 2"),
        "nonempty metrics",
    );
    refuses(
        &fixture!("technology").replace("warnings = 2", "warnings = 2\ncoverage = 85"),
        "conflicting threshold coverage",
    );
}

#[test]
fn required_gates_accumulate_and_removal_refuses() {
    let added = fixture!("technology")
        .replace(
            "gates = [\"lint\", \"coverage\"]",
            "gates = [\"lint\", \"coverage\", \"types\"]",
        )
        .replace(
            "[bindings.lint]",
            "[bindings.types]\nstate = \"unresolved\"\n[bindings.lint]",
        );
    let catalog = check_under(
        &tree().with("profiles/quality/technology.toml", &added),
        &Limits::PRODUCTION,
    )
    .unwrap();
    let profile = catalog
        .resources
        .iter()
        .find(|resource| resource.id.name == "technology")
        .unwrap();
    assert_eq!(
        profile.fields["gates"].texts().unwrap(),
        ["lint", "coverage", "types"]
    );
    refuses(fixture!("removed-gate"), "removed required gate coverage");
    refuses(
        &fixture!("technology").replace("gates = [\"lint\", \"coverage\"]", "gates = []"),
        "nonempty",
    );
}

#[test]
fn missing_required_binding_stays_unresolved() {
    let catalog = check_under(&tree(), &Limits::PRODUCTION).unwrap();
    let profile = catalog
        .resources
        .iter()
        .find(|resource| resource.id.name == "technology")
        .unwrap();
    let Value::Table(bindings) = &profile.fields["bindings"] else {
        panic!("bindings table")
    };
    let Value::Table(lint) = &bindings["lint"] else {
        panic!("binding table")
    };
    assert_eq!(lint["state"].text(), Some("unresolved"));
    assert!(!lint.contains_key("reference"));
    refuses(fixture!("false-binding"), "unknown variant");
    refuses(
        &fixture!("technology").replace("[bindings.lint]\nstate = \"unresolved\"\n", ""),
        "missing explicit binding for lint",
    );
    refuses(
        &fixture!("technology").replace("state = \"unresolved\"", "state = \"bound\""),
        "bound binding requires an exact reference",
    );
}

#[test]
fn baseline_identity_depth_and_declaration_refuse() {
    refuses(
        &fixture!("technology").replace(
            "baseline = \"quality-profile:common/baseline\"",
            "baseline = \"skill:common/valid-skill\"",
        ),
        "baseline must reference a quality-profile",
    );
    refuses(
        &fixture!("technology").replace(
            "requires = [\"quality-profile:common/baseline\"]",
            "requires = []",
        ),
        "must be declared in metadata.requires",
    );
    refuses(
        &fixture!("technology").replace(
            "quality-profile:common/baseline",
            "quality-profile:common/missing",
        ),
        "which does not exist",
    );
    let chained = tree().with(
        "profiles/quality/baseline.toml",
        &fixture!("baseline")
            .replace(
                "subject =",
                "baseline = \"quality-profile:common/technology\"\nsubject =",
            )
            .replace(
                "requires = []",
                "requires = [\"quality-profile:common/technology\"]",
            ),
    );
    let refusal = check_under(&chained, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(refusal.contains("baseline depth exceeds one"), "{refusal}");
}

#[test]
fn quality_shapes_and_binding_references_are_strict() {
    for (from, to, message) in [
        (
            "subject = \"synthetic-language\"",
            "subject = \"\"",
            "nonempty",
        ),
        (
            "[thresholds.floors]",
            "[thresholds]\nunknown = true\n[thresholds.floors]",
            "unknown field",
        ),
        (
            "state = \"unresolved\"",
            "state = \"unresolved\"\ncommand = \"run-check\"",
            "unknown field",
        ),
        (
            "[bindings.lint]",
            "[bindings.lint]\nreference = \"skill:common/valid-skill\"",
            "unresolved binding cannot claim a reference",
        ),
        (
            "warnings = 2",
            "warnings = \"two\"",
            "did not match any variant",
        ),
    ] {
        refuses(&fixture!("technology").replace(from, to), message);
    }
    refuses(
        &fixture!("technology").replace(
            "[bindings.lint]",
            "[bindings.extra]\nstate = \"unresolved\"\n[bindings.lint]",
        ),
        "binding for undeclared gate extra",
    );
    let malformed = fixture!("technology").replacen(
        "state = \"unresolved\"",
        "state = \"bound\"\nreference = \"not-an-id\"",
        1,
    );
    refuses(&malformed, "binding must be an exact typed reference");
    let bound = fixture!("technology").replacen(
        "state = \"unresolved\"",
        "state = \"bound\"\nreference = \"skill:common/valid-skill\"",
        1,
    );
    refuses(&bound, "must be declared in metadata.requires");
    let declared = bound.replace("requires = [", "requires = [\"skill:common/valid-skill\", ");
    assert!(
        check_under(
            &tree().with("profiles/quality/technology.toml", &declared),
            &Limits::PRODUCTION
        )
        .is_ok()
    );
    refuses(
        &declared.replace("skill:common/valid-skill", "skill:common/missing"),
        "which does not exist",
    );
}

#[test]
fn positional_thresholds_refuse() {
    let text = format!(
        "thresholds = [{{coverage = 90.0}}, {{warnings = 2}}]\n{}",
        fixture!("technology").replace(
            "[thresholds.floors]\ncoverage = 90.0\n[thresholds.ceilings]\nwarnings = 2\n",
            "",
        )
    );
    assert!(
        check_under(
            &tree().with("profiles/quality/technology.toml", &text),
            &Limits::PRODUCTION,
        )
        .is_err(),
        "positional thresholds accepted",
    );
}

#[test]
fn positional_binding_refuses() {
    let text = fixture!("bound").replace(
        "[bindings.lint]\nstate = \"bound\"\nreference = \"skill:common/valid-skill\"\n",
        "[bindings]\nlint = [\"bound\", \"skill:common/valid-skill\"]\n",
    );
    assert!(
        check_under(
            &tree().with("profiles/quality/technology.toml", &text),
            &Limits::PRODUCTION,
        )
        .is_err(),
        "positional binding accepted",
    );
}

#[test]
fn quality_integer_thresholds_never_round() {
    let large = tree()
        .edit(
            "profiles/quality/baseline.toml",
            "warnings = 5",
            "warnings = 9007199254740992",
        )
        .edit(
            "profiles/quality/technology.toml",
            "warnings = 2",
            "warnings = 9007199254740993",
        );
    let refusal = check_under(&large, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(refusal.contains("weakens ceiling warnings"), "{refusal}");
}

#[test]
fn quality_threshold_types_cannot_change() {
    refuses(
        &fixture!("technology").replace("warnings = 2", "warnings = 2.0"),
        "threshold warnings changes type from integer to float",
    );
    refuses(
        &fixture!("technology").replace("coverage = 90.0", "coverage = 90"),
        "threshold coverage changes type from float to integer",
    );
}

#[test]
fn quality_positive_and_refusal_fixtures_are_complete() {
    let bound = check_under(
        &tree().with("profiles/quality/technology.toml", fixture!("bound")),
        &Limits::PRODUCTION,
    );
    assert!(bound.is_ok(), "{bound:?}");
    for (text, message) in [
        (
            fixture!("missing-binding"),
            "missing explicit binding for lint",
        ),
        (fixture!("removed-floor"), "removed floor coverage"),
        (fixture!("removed-ceiling"), "removed ceiling warnings"),
        (
            fixture!("changed-type"),
            "threshold warnings changes type from integer to float",
        ),
        (fixture!("dangling-baseline"), "which does not exist"),
        (
            fixture!("undeclared-baseline"),
            "must be declared in metadata.requires",
        ),
        (fixture!("nonfinite"), "finite number"),
    ] {
        refuses(text, message);
    }
}

#[test]
fn quality_equal_bounds_and_required_lists() {
    let equal = fixture!("technology")
        .replace("coverage = 90.0", "coverage = 80.0")
        .replace("warnings = 2", "warnings = 5");
    assert!(
        check_under(
            &tree().with("profiles/quality/technology.toml", &equal),
            &Limits::PRODUCTION
        )
        .is_ok()
    );
    for (field, value) in [
        ("gates", "[\"lint\", \"coverage\"]"),
        ("applicability", "[\"all\"]"),
        (
            "failure_conditions",
            "[\"missing-report\", \"gate-failed\"]",
        ),
        ("evidence", "[\"json-report\"]"),
    ] {
        refuses(
            &fixture!("technology")
                .replace(&format!("{field} = {value}"), &format!("{field} = []")),
            "nonempty list",
        );
        refuses(
            &fixture!("technology").replace(&format!("{field} = {value}\n"), ""),
            &format!("{field}: missing"),
        );
    }
}

#[test]
fn quality_same_type_intersections() {
    refuses(
        &fixture!("technology").replace("warnings = 2", "warnings = 2\ncoverage = 85.0"),
        "conflicting threshold coverage",
    );
    for ceiling in ["90.0", "95.0"] {
        let text = fixture!("technology").replace(
            "warnings = 2",
            &format!("warnings = 2\ncoverage = {ceiling}"),
        );
        assert!(
            check_under(
                &tree().with("profiles/quality/technology.toml", &text),
                &Limits::PRODUCTION,
            )
            .is_ok()
        );
    }
}

#[test]
fn quality_acyclic_depth_refuses() {
    let middle = fixture!("technology").replace("name = \"technology\"", "name = \"middle\"");
    let child = fixture!("technology").replace(
        "quality-profile:common/baseline",
        "quality-profile:common/middle",
    );
    let input = tree()
        .with("profiles/quality/middle.toml", &middle)
        .with("profiles/quality/technology.toml", &child);
    let refusal = check_under(&input, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(refusal.contains("baseline depth exceeds one"), "{refusal}");
}

#[test]
fn quality_integer_floors_only_narrow() {
    for (baseline, child, admitted) in [
        (80_i64, 80_i64, true),
        (80, 81, true),
        (80, 79, false),
        (9_007_199_254_740_996, 9_007_199_254_740_996, true),
        (9_007_199_254_740_996, 9_007_199_254_740_997, true),
        (9_007_199_254_740_996, 9_007_199_254_740_995, false),
    ] {
        let input = tree()
            .edit(
                "profiles/quality/baseline.toml",
                "coverage = 80.0",
                &format!("coverage = {baseline}"),
            )
            .edit(
                "profiles/quality/technology.toml",
                "coverage = 90.0",
                &format!("coverage = {child}"),
            );
        let result = check_under(&input, &Limits::PRODUCTION);
        if admitted {
            assert!(result.is_ok(), "{baseline} -> {child}: {result:?}");
        } else {
            let refusal = result.unwrap_err().to_string();
            assert!(refusal.contains("weakens floor coverage"), "{refusal}");
        }
    }
}

#[test]
fn quality_float_ceilings_only_narrow() {
    for (ceiling, admitted) in [("5.0", true), ("2.0", true), ("6.0", false)] {
        let input = tree()
            .edit(
                "profiles/quality/baseline.toml",
                "warnings = 5",
                "warnings = 5.0",
            )
            .edit(
                "profiles/quality/technology.toml",
                "warnings = 2",
                &format!("warnings = {ceiling}"),
            );
        let result = check_under(&input, &Limits::PRODUCTION);
        if admitted {
            assert!(result.is_ok(), "{ceiling}: {result:?}");
        } else {
            let refusal = result.unwrap_err().to_string();
            assert!(refusal.contains("weakens ceiling warnings"), "{refusal}");
        }
    }
}
