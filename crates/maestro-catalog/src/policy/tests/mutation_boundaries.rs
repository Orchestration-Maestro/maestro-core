//! Cedar diagnostic and aggregate boundaries are observable decisions.
use crate::{
    limits::Limits,
    policy::{Cedar, Decision, NoFacts, Operation, PolicyChecker, load, test_cases},
};
use maestro_test_scratch::scratch_directory;
use std::fs;

const SCHEMA: &str = include_str!("../../../../../tests/fixtures/catalog/policy/schema.json");

#[test]
fn absent_fact_diagnostic_only_appears_for_a_denial() {
    let operation = Operation {
        action: "read".into(),
        target: "public".into(),
    };
    let checker = Cedar::new(SCHEMA, "permit(principal, action, resource);").unwrap();
    let allowed = checker.check(&operation, &NoFacts);
    assert_eq!(allowed.decision, Decision::Allow);
    assert!(allowed.diagnostics.is_empty(), "{allowed:?}");
    let denied = Cedar::new(SCHEMA, "").unwrap().check(&operation, &NoFacts);
    assert_eq!(denied.decision, Decision::Deny);
    assert_eq!(denied.diagnostics, ["needs trusted host facts (C20)"]);
}

#[test]
fn empty_action_or_target_is_rejected_before_cedar() {
    let checker = Cedar::new(SCHEMA, "permit(principal, action, resource);").unwrap();
    for (action, target) in [(" ", "public"), ("read", " ")] {
        let result = checker.check(
            &Operation {
                action: action.into(),
                target: target.into(),
            },
            &NoFacts,
        );
        assert_eq!(result.decision, Decision::Deny);
        assert_eq!(result.diagnostics, ["empty operation action or target"]);
    }
}

#[test]
fn policy_loading_accepts_exact_count_and_aggregate_and_refuses_one_over() {
    let root = scratch_directory().unwrap();
    fs::write(root.join("schema.json"), SCHEMA).unwrap();
    let first = format!(
        "permit(principal, action, resource);\n//{}",
        "a".repeat(5000)
    );
    let second = format!(
        "forbid(principal, action, resource);\n//{}",
        "b".repeat(5000)
    );
    fs::write(root.join("one.cedar"), &first).unwrap();
    fs::write(root.join("two.cedar"), &second).unwrap();
    let total = u64::try_from(first.len() + second.len() + 2).unwrap();
    let limits = Limits {
        catalog_resources: 2,
        source_file_bytes: total,
        ..Limits::PRODUCTION
    };
    load(&root, &limits).unwrap();
    assert!(
        load(
            &root,
            &Limits {
                catalog_resources: 1,
                ..limits
            }
        )
        .unwrap_err()
        .contains("too many")
    );
    assert!(
        load(
            &root,
            &Limits {
                source_file_bytes: total - 1,
                ..limits
            }
        )
        .unwrap_err()
        .contains("combined Cedar policies")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn authoring_cases_refuse_empty_and_duplicate_names_independently() {
    let root = scratch_directory().unwrap();
    fs::write(root.join("schema.json"), SCHEMA).unwrap();
    for names in [vec![" "], vec!["same", "same"]] {
        let cases: Vec<_> = names
            .iter()
            .map(|name| {
                serde_json::json!({
                    "name": name,
                    "operation": {"action": "read", "target": "public"},
                    "facts": null,
                    "expected": "deny"
                })
            })
            .collect();
        fs::write(root.join("cases.json"), serde_json::to_vec(&cases).unwrap()).unwrap();
        assert!(
            test_cases(&root, &Limits::PRODUCTION)
                .unwrap_err()
                .contains("empty or duplicate")
        );
    }
    fs::remove_dir_all(root).unwrap();
}
