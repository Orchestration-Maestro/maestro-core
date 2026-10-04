//! C64 folder handoffs and complete inert eval declarations.

use super::{
    contracts::valid,
    support::{assert_refused, check_under},
};
use crate::limits::Limits;

#[test]
fn c64_handoff_folder_layout_accepts() {
    check_under(&valid(), &Limits::PRODUCTION).unwrap();
    assert_refused(vec![
        (
            "missing sidecar",
            valid().without("core/handoffs/synthetic/handoff.maestro.toml"),
            "no handoff.maestro.toml sidecar",
        ),
        (
            "flat handoff",
            valid()
                .without("core/handoffs/synthetic/handoff.md")
                .with("core/handoffs/synthetic.handoff.md", "flat"),
            "not a registered v4 placement",
        ),
    ]);
}

#[test]
fn c64_eval_missing_driver_refuses() {
    assert_refused(vec![(
        "missing driver",
        valid().edit("evals/synthetic.toml", "driver = \"synthetic\"\n", ""),
        "driver: missing",
    )]);
}

#[test]
fn c64_eval_full_declaration_accepts() {
    check_under(&valid(), &Limits::PRODUCTION).unwrap();
    for timeout in [1, 1_800_000] {
        for status in ["passed", "failed"] {
            check_under(
                &valid()
                    .edit("evals/synthetic.toml", "1800000", &timeout.to_string())
                    .edit(
                        "evals/synthetic.toml",
                        "\"passed\"",
                        &format!("\"{status}\""),
                    ),
                &Limits::PRODUCTION,
            )
            .unwrap();
        }
    }
}

#[test]
fn c64_eval_expectations_match_case_order() {
    let tree = valid().with("evals/second.json", "[]").edit(
        "evals/synthetic.toml",
        "cases = [\"eval-cases.json\"]",
        "cases = [\"eval-cases.json\", \"second.json\"]",
    );
    assert_refused(vec![("too few expectations", tree.clone(), "one per case")]);
    let complete = tree
        .edit(
            "evals/synthetic.toml",
            "outputs = [\"eval-outputs.json\"]",
            "outputs = [\"eval-outputs.json\", \"eval-outputs.json\"]",
        )
        .edit(
            "evals/synthetic.toml",
            "statuses = [\"passed\"]",
            "statuses = [\"passed\", \"passed\"]",
        );
    check_under(&complete, &Limits::PRODUCTION).unwrap();
    for (field, declaration) in [
        ("expected.outputs", "outputs = [\"eval-outputs.json\"]\n"),
        ("expected.statuses", "statuses = [\"passed\"]\n"),
    ] {
        assert_refused(vec![(
            "missing expectation",
            valid().edit("evals/synthetic.toml", declaration, ""),
            &format!("{field}: missing"),
        )]);
    }
    let empty = valid()
        .edit(
            "evals/synthetic.toml",
            "cases = [\"eval-cases.json\"]",
            "cases = []",
        )
        .edit(
            "evals/synthetic.toml",
            "outputs = [\"eval-outputs.json\"]",
            "outputs = []",
        );
    assert_refused(vec![(
        "empty expectations with empty cases",
        empty,
        "expected.outputs:",
    )]);
}

#[test]
fn c64_eval_required_fields_refuse() {
    for (field, declaration) in [
        ("subjects", "subjects = [\"prompt:common/synthetic\"]\n"),
        ("inputs", "inputs = [\"eval-inputs.json\"]\n"),
        ("cases", "cases = [\"eval-cases.json\"]\n"),
        ("checks", "checks = [\"standard-check:security/shape\"]\n"),
        (
            "expected",
            "[expected]\noutputs = [\"eval-outputs.json\"]\nstatuses = [\"passed\"]\n",
        ),
        ("limits", "[limits]\ntimeout_ms = 1800000\n"),
    ] {
        assert_refused(vec![(
            "missing declaration",
            valid().edit("evals/synthetic.toml", declaration, ""),
            &format!("{field}: missing"),
        )]);
    }
}

#[test]
fn c64_eval_static_guards_refuse() {
    for (from, to, expected) in [
        (
            "driver = \"synthetic\"",
            "driver = \"../runner\"",
            "driver:",
        ),
        (
            "subjects = [\"prompt:common/synthetic\"]",
            "subjects = []",
            "subjects:",
        ),
        (
            "subjects = [\"prompt:common/synthetic\"]",
            "subjects = [\"bad\"]",
            "subjects:",
        ),
        (
            "subjects = [\"prompt:common/synthetic\"]",
            "subjects = [\"prompt:common/absent\"]",
            "must be declared in metadata.requires",
        ),
        (
            "checks = [\"standard-check:security/shape\"]",
            "checks = []",
            "checks:",
        ),
        (
            "checks = [\"standard-check:security/shape\"]",
            "checks = [\"prompt:common/synthetic\"]",
            "checks:",
        ),
        (
            "checks = [\"standard-check:security/shape\"]",
            "checks = [\"standard-check:security/absent\"]",
            "must be declared in metadata.requires",
        ),
        ("inputs = [\"eval-inputs.json\"]", "inputs = []", "inputs:"),
        ("cases = [\"eval-cases.json\"]", "cases = []", "cases:"),
        (
            "outputs = [\"eval-outputs.json\"]",
            "outputs = []",
            "expected.outputs:",
        ),
        (
            "statuses = [\"passed\"]",
            "statuses = []",
            "expected.statuses:",
        ),
        (
            "statuses = [\"passed\"]",
            "statuses = [\"unknown\"]",
            "expected.statuses:",
        ),
        (
            "statuses = [\"passed\"]",
            "statuses = [\"passed\", \"failed\"]",
            "one per case",
        ),
        (
            "outputs = [\"eval-outputs.json\"]",
            "outputs = [\"eval-outputs.json\", \"eval-inputs.json\"]",
            "one per case",
        ),
        (
            "timeout_ms = 1800000",
            "timeout_ms = 0",
            "limits.timeout_ms:",
        ),
        (
            "timeout_ms = 1800000",
            "timeout_ms = 1800001",
            "limits.timeout_ms:",
        ),
        (
            "timeout_ms = 1800000",
            "timeout_ms = 1.5",
            "limits.timeout_ms:",
        ),
        ("timeout_ms = 1800000", "", "limits.timeout_ms: missing"),
        (
            "[limits]",
            "[limits]\nunknown = true",
            "limits.unknown: unknown key",
        ),
        (
            "[expected]",
            "[expected]\nunknown = true",
            "expected.unknown: unknown key",
        ),
    ] {
        assert_refused(vec![(
            "invalid declaration",
            valid().edit("evals/synthetic.toml", from, to),
            expected,
        )]);
    }
}

#[test]
fn c64_eval_dangling_declared_references_refuse() {
    for path in [
        "prompts/synthetic.prompt.md",
        "standards/security/checks/shape.toml",
    ] {
        assert_refused(vec![(
            "dangling declared reference",
            if path.starts_with("prompts/") {
                valid()
                    .without(path)
                    .without("prompts/synthetic.maestro.toml")
            } else {
                valid().without(path)
            },
            "which does not exist",
        )]);
    }
}

#[test]
fn c64_eval_all_assets_refuse_unsafe_paths() {
    for asset in ["eval-inputs.json", "eval-cases.json", "eval-outputs.json"] {
        for path in [
            "../outside.json",
            "/outside.json",
            "https://example.invalid/a.json",
            "*.json",
            "a\\\\b.json",
            "data.txt",
        ] {
            assert_refused(vec![(
                "unsafe asset",
                valid().edit("evals/synthetic.toml", asset, path),
                "exact",
            )]);
        }
        assert_refused(vec![(
            "missing asset",
            valid().without(&format!("evals/{asset}")),
            "cannot read inventoried asset",
        )]);
    }
}

#[test]
fn c64_missing_contract_bindings_refuse() {
    for path in [
        "prompts/synthetic.prompt.md",
        "core/handoffs/synthetic/handoff.md",
        "evals/synthetic.toml",
    ] {
        for field in ["input", "output"] {
            let declaration = if path.ends_with("toml") {
                format!("{field}_contract = \"contract:common/{field}\"\n")
            } else {
                format!("{field}_contract: contract:common/{field}\n")
            };
            assert_refused(vec![(
                "missing binding",
                valid().edit(path, &declaration, ""),
                &format!("{field}_contract: missing"),
            )]);
        }
    }
}
