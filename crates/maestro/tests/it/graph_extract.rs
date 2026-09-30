//! Model graph extraction is an explicit, mutually exclusive build mode.

use std::process::Command;

#[test]
fn graph_build_help_documents_explicit_model_inputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["knowledge", "graph", "build", "--help"])
        .output()
        .expect("synthetic fixture is valid");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("synthetic fixture is valid");
    for flag in [
        "--rule",
        "--extractor-card",
        "--window-policy",
        "--token-budget",
    ] {
        assert!(help.contains(flag), "missing {flag} in {help}");
    }
    assert!(
        help.contains("crash retry"),
        "retry bound is absent: {help}"
    );
}

#[test]
fn graph_build_refuses_mixing_rule_and_model_inputs_before_any_model_call() {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args([
            "knowledge",
            "graph",
            "build",
            "--collection",
            "synthetic",
            "--rule",
            "rule.json",
            "--extractor-card",
            &"a".repeat(64),
            "--window-policy",
            "policy.json",
            "--token-budget",
            "100",
        ])
        .output()
        .expect("synthetic fixture is valid");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
}
