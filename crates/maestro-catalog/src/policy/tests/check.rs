//! Allow/deny/error neighbours reach Cedar, never an effect executor.

use crate::{
    limits::Limits,
    policy::{
        Cedar, Decision, HostFacts, NoFacts, Operation, PolicyChecker, TrustedFacts, load,
        test_cases,
    },
};
use std::{cell::Cell, path::PathBuf};

/// Public synthetic authoring policy inputs.
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("tests/fixtures/catalog/policy")
}

/// A host double supplying facts separately from the operation.
#[derive(Debug)]
struct Host {
    /// Separate synthetic host facts.
    facts: Option<TrustedFacts>,
    /// Executor spy: not exposed by the `HostFacts` port.
    executor_calls: Cell<usize>,
}

impl Host {
    /// A host with a fresh executor spy.
    fn new(facts: Option<TrustedFacts>) -> Self {
        Self {
            facts,
            executor_calls: Cell::new(0),
        }
    }

    /// Calibrates the spy after checking; does not execute an actual effect.
    fn execute_spy(&self) {
        self.executor_calls.set(self.executor_calls.get() + 1);
    }
}

impl HostFacts for Host {
    fn facts(&self, _operation: &Operation) -> Option<TrustedFacts> {
        self.facts.clone()
    }
}

#[test]
fn each_rule_reaches_real_cedar_with_allowed_and_denied_neighbours() {
    let checker = load(&fixtures(), &Limits::PRODUCTION).unwrap();
    let cases = test_cases(&fixtures(), &Limits::PRODUCTION).unwrap();
    for case in &cases {
        let host = Host::new(case.facts.clone());
        let result = checker.check(&case.operation, &host);
        assert_eq!(result.decision, case.expected, "{}: {result:?}", case.name);
        // This is a check, not execution admission: even allow must not call it.
        assert_eq!(
            host.executor_calls.get(),
            0,
            "{} called the executor",
            case.name
        );
        host.execute_spy();
        assert_eq!(host.executor_calls.get(), 1);
        if result.decision == Decision::Allow {
            assert!(!result.policies.is_empty(), "{} skipped Cedar", case.name);
        }
    }
    assert_eq!(cases.len(), 12);
}

#[test]
fn schema_and_policy_type_errors_refuse_before_evaluation() {
    let schema = include_str!("../../../../../tests/fixtures/catalog/policy/schema.json");
    assert!(Cedar::new("{}", "permit(principal, action, resource);").is_err());
    assert!(Cedar::new(schema, "not a Cedar policy").is_err());
    assert!(
        Cedar::new(
            schema,
            "permit(principal, action, resource) when { context.trusted == 7 };"
        )
        .is_err()
    );
}

#[test]
fn evaluation_errors_deny_even_beside_a_matching_permit() {
    let checker = Cedar::new(
        include_str!("../../../../../tests/fixtures/catalog/policy/schema.json"),
        concat!(
            "permit(principal, action, resource); ",
            "permit(principal, action, resource) when { 9223372036854775807 + 1 > 0 };"
        ),
    )
    .unwrap();
    let host = Host::new(None);
    let result = checker.check(
        &Operation {
            action: "read".into(),
            target: "public".into(),
        },
        &host,
    );
    assert_eq!(result.decision, Decision::Deny, "{result:?}");
    assert!(!result.diagnostics.is_empty());
    assert_eq!(host.executor_calls.get(), 0);
    host.execute_spy();
    assert_eq!(host.executor_calls.get(), 1);
}

#[test]
fn trusted_facts_are_bound_to_the_exact_operation() {
    let checker = load(&fixtures(), &Limits::PRODUCTION).unwrap();
    let cases = test_cases(&fixtures(), &Limits::PRODUCTION).unwrap();
    let allowed = cases
        .iter()
        .find(|case| case.name == "destructive-allow")
        .unwrap();
    for field in ["actor", "operation", "target"] {
        let mut facts = allowed.facts.clone().unwrap();
        match field {
            "actor" => facts.actor.clear(),
            "operation" => facts.operation = "read".into(),
            _ => facts.target = "another-document".into(),
        }
        let result = checker.check(&allowed.operation, &Host::new(Some(facts)));
        assert_eq!(result.decision, Decision::Deny, "{field}: {result:?}");
        assert!(!result.diagnostics.is_empty());
    }
}

#[test]
fn bounded_operation_data_cannot_supply_trusted_facts() {
    for field in [
        "actor",
        "approved",
        "protected",
        "egress",
        "tool",
        "facts",
        "context",
    ] {
        let text = format!(r#"{{"action":"read","target":"public","{field}":true}}"#);
        assert!(
            Operation::parse(&text, &Limits::PRODUCTION).is_err(),
            "{field}"
        );
    }
    assert!(
        Operation::parse(
            r#"{"action":"read","action":"delete","target":"public"}"#,
            &Limits::PRODUCTION
        )
        .is_err()
    );
    let text = r#"{"action":"read","target":"public"}"#;
    let limits = Limits {
        source_file_bytes: text.len() as u64,
        ..Limits::PRODUCTION
    };
    assert!(Operation::parse(text, &limits).is_ok());
    assert!(Operation::parse(&format!("{text} "), &limits).is_err());
}

#[test]
fn approval_needed_is_not_permission_and_protection_still_denies() {
    let checker = load(&fixtures(), &Limits::PRODUCTION).unwrap();
    let cases = test_cases(&fixtures(), &Limits::PRODUCTION).unwrap();
    let case = cases
        .iter()
        .find(|case| case.name == "destructive-approval")
        .unwrap();
    let host = Host::new(case.facts.clone());
    assert_eq!(
        checker.check(&case.operation, &host).decision,
        Decision::ApprovalNeeded
    );
    let mut facts = case.facts.clone().unwrap();
    facts.protected = true;
    assert_eq!(
        checker
            .check(&case.operation, &Host::new(Some(facts)))
            .decision,
        Decision::Deny
    );
}

#[test]
fn normalized_identifiers_are_data_and_empty_fields_refuse() {
    let checker = load(&fixtures(), &Limits::PRODUCTION).unwrap();
    for (action, target) in [
        ("", "public"),
        ("read", ""),
        ("read", "public\" || true || \""),
        ("read", "public\\\""),
    ] {
        let host = Host::new(None);
        let result = checker.check(
            &Operation {
                action: action.into(),
                target: target.into(),
            },
            &host,
        );
        assert_eq!(result.decision, Decision::Deny, "{result:?}");
        assert_eq!(host.executor_calls.get(), 0);
    }
}

#[test]
fn bounded_loaders_and_stdin_refuse_one_past_the_limit() {
    use crate::policy::read_input;
    use std::io::{self, Read};

    /// A broken input stream, to test a real read failure without an effect.
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("synthetic read error"))
        }
    }
    let limits = Limits {
        source_file_bytes: 2,
        ..Limits::PRODUCTION
    };
    assert_eq!(read_input("{}".as_bytes(), &limits).unwrap(), "{}");
    assert!(read_input("{} ".as_bytes(), &limits).is_err());
    assert!(read_input([0xff].as_slice(), &limits).is_err());
    assert!(read_input(Broken, &limits).is_err());
    assert!(load(&fixtures(), &limits).is_err());
    let limits = Limits {
        catalog_resources: 12,
        ..Limits::PRODUCTION
    };
    assert!(test_cases(&fixtures(), &limits).is_ok());
    assert!(
        test_cases(
            &fixtures(),
            &Limits {
                catalog_resources: 11,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        load(
            &fixtures(),
            &Limits {
                catalog_resources: 0,
                ..limits
            }
        )
        .is_err()
    );
}

#[test]
fn all_policy_parse_diagnostics_are_retained() {
    let error = Cedar::new(
        include_str!("../../../../../tests/fixtures/catalog/policy/schema.json"),
        "permit(principal, action, resource) when { unknown_one }; \
         permit(principal, action, resource) when { unknown_two };",
    )
    .unwrap_err();
    assert!(error.contains("unknown_one"), "{error}");
    assert!(error.contains("unknown_two"), "{error}");
}

#[test]
fn identity_dependent_reads_explain_absent_host_facts() {
    let checker = Cedar::new(
        include_str!("../../../../../tests/fixtures/catalog/policy/schema.json"),
        "permit(principal == Actor::\"local-user\", action == Action::\"read\", resource);",
    )
    .unwrap();
    let result = checker.check(
        &Operation {
            action: "read".into(),
            target: "public".into(),
        },
        &NoFacts,
    );
    assert_eq!(result.decision, Decision::Deny);
    assert_eq!(result.diagnostics, ["needs trusted host facts (C20)"]);
}

#[test]
fn policy_schema_and_cases_enforce_exact_container_depth() {
    let schema_limits = Limits {
        source_depth: 8,
        ..Limits::PRODUCTION
    };
    assert!(load(&fixtures(), &schema_limits).is_ok());
    let error = load(
        &fixtures(),
        &Limits {
            source_depth: 7,
            ..schema_limits
        },
    )
    .unwrap_err();
    assert!(error.contains("depth"), "{error}");
    let case_limits = Limits {
        source_depth: 3,
        ..Limits::PRODUCTION
    };
    assert!(test_cases(&fixtures(), &case_limits).is_ok());
    let error = test_cases(
        &fixtures(),
        &Limits {
            source_depth: 2,
            ..case_limits
        },
    )
    .unwrap_err();
    assert!(error.contains("depth"), "{error}");
}

#[test]
fn policy_json_depth_counts_containers_not_quoted_or_escaped_brackets() {
    let limits = Limits {
        source_depth: 1,
        ..Limits::PRODUCTION
    };
    let text = r#"{"action":"read[{}]\\\"","target":"[public]"}"#;
    assert!(Operation::parse(text, &limits).is_ok());
    let error = Operation::parse(
        text,
        &Limits {
            source_depth: 0,
            ..limits
        },
    )
    .unwrap_err();
    assert!(error.contains("depth"), "{error}");
}
