//! Real Cedar through the effect-free policy CLI.

use super::support::Home;
use std::{fs, io::Write as _, path::PathBuf};

/// The synthetic policy directory.
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("tests/fixtures/catalog/policy")
}

#[test]
fn catalog_policy_test_runs_real_allow_and_deny_neighbours() {
    let home = Home::bare();
    let result = home.run(&[
        "--json",
        "policy",
        "test",
        "--catalog-dir",
        fixtures().to_str().unwrap(),
    ]);
    assert_eq!(result.code, Some(0), "{result:?}");
    assert_eq!(result.json()["passed"], 12);
}

#[test]
fn catalog_policy_check_allows_only_fact_free_neighbour() {
    let home = Home::bare();
    for (input, decision, code) in [
        (r#"{"action":"read","target":"public"}"#, "allow", 0),
        (r#"{"action":"delete","target":"document"}"#, "deny", 2),
        (r#"{"action":"read","target":"unknown"}"#, "deny", 2),
        (
            r#"{"action":"delete","target":"document","approved":true,"actor":"local-user"}"#,
            "deny",
            2,
        ),
    ] {
        let (running, mut stdin) = home.start_with_stdin(&[
            "--json",
            "policy",
            "check",
            "--stdin",
            "--catalog-dir",
            fixtures().to_str().unwrap(),
        ]);
        stdin.write_all(input.as_bytes()).unwrap();
        drop(stdin);
        let result = running.finish();
        assert_eq!(result.code, Some(code), "{result:?}");
        assert_eq!(result.json()["decision"], decision);
        if input.contains("delete") && !input.contains("approved") {
            assert!(
                result.stdout.contains("needs trusted host facts (C20)"),
                "{result:?}"
            );
        }
        assert!(fs::read_dir(home.data()).unwrap().next().is_none());
    }
}

#[test]
fn catalog_policy_test_refuses_zero_discovered_cases() {
    let home = Home::bare();
    let root = home.root().join("policy");
    fs::create_dir(&root).unwrap();
    for file in ["schema.json", "rules.cedar"] {
        fs::copy(fixtures().join(file), root.join(file)).unwrap();
    }
    fs::write(root.join("cases.json"), "[]").unwrap();
    let result = home.run(&["policy", "test", "--catalog-dir", root.to_str().unwrap()]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(
        result.stderr.contains("zero policy test cases"),
        "{result:?}"
    );
}

/// Copies fixture policy inputs under the synthetic catalog's production layout.
fn core_catalog(home: &Home) -> PathBuf {
    let root = home.root().join("catalog");
    let policies = root.join("core/policies");
    let scenarios = root.join("core/evals/scenarios");
    fs::create_dir_all(&policies).unwrap();
    fs::create_dir_all(&scenarios).unwrap();
    fs::copy(
        fixtures().join("schema.json"),
        policies.join("schema.cedarschema.json"),
    )
    .unwrap();
    fs::copy(fixtures().join("rules.cedar"), policies.join("rules.cedar")).unwrap();
    fs::copy(
        fixtures().join("cases.json"),
        scenarios.join("policy-neighbours.json"),
    )
    .unwrap();
    root
}

#[test]
fn catalog_policy_core_layout_and_all_mismatches_are_checked() {
    let home = Home::bare();
    let root = core_catalog(&home);
    let args = [
        "--json",
        "policy",
        "test",
        "--catalog-dir",
        root.to_str().unwrap(),
    ];
    let result = home.run(&args);
    assert_eq!(result.code, Some(0), "{result:?}");
    let path = root.join("core/evals/scenarios/policy-neighbours.json");
    let mut cases: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    cases[0]["expected"] = "deny".into();
    cases[1]["expected"] = "allow".into();
    fs::write(&path, serde_json::to_vec(&cases).unwrap()).unwrap();
    let result = home.run(&args);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert_eq!(result.json()["passed"], 10);
    assert_eq!(result.json()["diagnostics"].as_array().unwrap().len(), 2);
    assert!(result.stdout.contains("default-deny-allow"));
    assert!(result.stdout.contains("default-deny-refuse"));
}

#[test]
fn catalog_policy_check_schema_and_evaluation_errors_deny() {
    let home = Home::bare();
    let root = core_catalog(&home);
    let rules = root.join("core/policies/rules.cedar");
    for text in [
        concat!(
            "permit(principal, action, resource); ",
            "permit(principal, action, resource) when { 9223372036854775807 + 1 > 0 };"
        ),
        "permit(principal, action, resource) when { context.trusted == 7 };",
        "invalid policy",
    ] {
        fs::write(&rules, text).unwrap();
        let (running, mut stdin) = home.start_with_stdin(&[
            "--json",
            "policy",
            "check",
            "--stdin",
            "--catalog-dir",
            root.to_str().unwrap(),
        ]);
        stdin
            .write_all(br#"{"action":"read","target":"public"}"#)
            .unwrap();
        drop(stdin);
        let result = running.finish();
        assert_eq!(result.code, Some(2), "{result:?}");
        assert_eq!(result.json()["decision"], "deny");
        assert!(!result.json()["diagnostics"].as_array().unwrap().is_empty());
        assert!(fs::read_dir(home.data()).unwrap().next().is_none());
    }
}

#[test]
fn catalog_policy_missing_malformed_and_duplicate_cases_refuse() {
    let home = Home::bare();
    let root = core_catalog(&home);
    let path = root.join("core/evals/scenarios/policy-neighbours.json");
    for text in [
        "not json",
        concat!(
            r#"[{"name":"","operation":{"action":"read","target":"public"},"#,
            r#""facts":null,"expected":"allow"}]"#
        ),
        concat!(
            r#"[{"name":"duplicate","operation":{"action":"read","target":"public"},"#,
            r#""facts":null,"expected":"allow"},{"name":"duplicate","#,
            r#""operation":{"action":"read","target":"public"},"facts":null,"expected":"allow"}]"#
        ),
    ] {
        fs::write(&path, text).unwrap();
        let result = home.run(&["policy", "test", "--catalog-dir", root.to_str().unwrap()]);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(!result.stderr.is_empty());
    }
    fs::remove_file(path).unwrap();
    let result = home.run(&["policy", "test", "--catalog-dir", root.to_str().unwrap()]);
    assert_eq!(result.code, Some(2), "{result:?}");
}
