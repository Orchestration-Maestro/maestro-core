//! Prompt, handoff and eval declarations through the real source checker.

use super::{
    registry::from_data,
    support::{MemoryTree, assert_refused, assert_refused_by, check_by, check_under},
};
use crate::{limits::Limits, source::ResourceId};
use std::path::Path;

/// The minimal authored synthetic contract set.
macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/catalog/contracts/",
            $name
        ))
    };
}

/// A common prompt and eval, plus a core handoff using common contracts.
fn valid() -> MemoryTree {
    let metadata = fixture!("input.maestro.toml").replace(
        "requires = []",
        "requires = [\"contract:common/input\", \"contract:common/output\"]",
    );
    MemoryTree::valid()
        .with("prompts/synthetic.prompt.md", fixture!("prompt-valid.md"))
        .with("prompts/synthetic.maestro.toml", &metadata)
        .with(
            "core/handoffs/synthetic.handoff.md",
            fixture!("handoff-valid.md"),
        )
        .with(
            "core/handoffs/synthetic.maestro.toml",
            &metadata.replace(
                "\"contract:common/input\",",
                "\"agent:core/valid\", \"contract:common/input\",",
            ),
        )
        .with("contracts/input.schema.json", fixture!("input.schema.json"))
        .with(
            "contracts/input.maestro.toml",
            fixture!("input.maestro.toml"),
        )
        .with(
            "contracts/output.schema.json",
            fixture!("output.schema.json"),
        )
        .with(
            "contracts/output.maestro.toml",
            fixture!("output.maestro.toml"),
        )
        .with("evals/synthetic.toml", fixture!("eval-valid.toml"))
        .with("evals/eval-cases.json", fixture!("eval-cases.json"))
}

#[test]
fn common_contract_reference_accepts() {
    let catalog = check_under(&valid(), &Limits::PRODUCTION).unwrap();
    for id in [
        "prompt:common/synthetic",
        "handoff:core/synthetic",
        "eval-case:common/synthetic",
    ] {
        assert!(
            catalog
                .resources
                .iter()
                .any(|resource| resource.id == ResourceId::parse(id).unwrap())
        );
    }
    let eval = catalog
        .resources
        .iter()
        .find(|resource| resource.id.kind == "eval-case")
        .unwrap();
    assert_eq!(eval.data, ["evals/eval-cases.json"]);
}

#[test]
fn unfilled_prompt_template_refuses() {
    check_under(&valid(), &Limits::PRODUCTION).unwrap();
    for token in ["{{record}}", "${record}", "{{", "${"] {
        assert_refused(vec![(
            "unfilled prompt",
            valid().edit(
                "prompts/synthetic.prompt.md",
                "supplied synthetic record",
                token,
            ),
            "prompts/synthetic.prompt.md: body: unfilled template",
        )]);
    }
    check_under(
        &valid().edit(
            "prompts/synthetic.prompt.md",
            "supplied synthetic record",
            "<record>",
        ),
        &Limits::PRODUCTION,
    )
    .unwrap();
}

#[test]
fn handoff_missing_section_refuses() {
    check_under(&valid(), &Limits::PRODUCTION).unwrap();
    for section in ["Inputs", "Context", "Deliverables", "Acceptance"] {
        assert_refused(vec![(
            "missing section",
            valid().edit(
                "core/handoffs/synthetic.handoff.md",
                &format!("## {section}"),
                &format!("### {section}"),
            ),
            "core/handoffs/synthetic.handoff.md: body: expected the sections",
        )]);
    }
    assert_refused(vec![(
        "empty section",
        valid().edit(
            "core/handoffs/synthetic.handoff.md",
            "Return the declared output shape.",
            "",
        ),
        "section \"Acceptance\" is empty",
    )]);
}

/// Mutate a primary contract field, not the eval's embedded requires neighbour.
fn contract_field(path: &str, field: &str, reference: &str) -> MemoryTree {
    let (from, to) = if Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
    {
        (
            format!("{field}_contract = \"contract:common/{field}\""),
            format!("{field}_contract = \"{reference}\""),
        )
    } else {
        (
            format!("{field}_contract: contract:common/{field}"),
            format!("{field}_contract: {reference}"),
        )
    };
    valid().edit(path, &from, &to)
}

#[test]
fn external_contract_reference_refuses() {
    check_under(&valid(), &Limits::PRODUCTION).unwrap();
    for path in [
        "prompts/synthetic.prompt.md",
        "core/handoffs/synthetic.handoff.md",
        "evals/synthetic.toml",
    ] {
        for reference in [
            "https://example.invalid/schema",
            "../contracts/input.schema.json",
            "agent:core/valid",
            "contract:input",
            "contract:common/input#fragment",
        ] {
            assert_refused(vec![(
                "external reference",
                contract_field(path, "input", reference),
                "needs a qualified contract ID",
            )]);
        }
    }
    assert_refused(vec![(
        "external JSON reference",
        valid().edit(
            "contracts/input.schema.json",
            "\"type\": \"string\"",
            "\"$ref\": \"https://example.invalid/schema\"",
        ),
        "paths and external retrieval refuse",
    )]);
}

#[test]
fn dangling_contract_refuses() {
    check_under(&valid(), &Limits::PRODUCTION).unwrap();
    for path in [
        "prompts/synthetic.prompt.md",
        "core/handoffs/synthetic.handoff.md",
        "evals/synthetic.toml",
    ] {
        for field in ["input", "output"] {
            assert_refused(vec![(
                "undeclared reference",
                contract_field(path, field, "contract:common/absent"),
                "must be declared in metadata.requires",
            )]);
        }
    }
    assert_refused(vec![(
        "dangling declared reference",
        valid()
            .without("contracts/input.schema.json")
            .without("contracts/input.maestro.toml"),
        "names contract:common/input, which does not exist",
    )]);
}

#[test]
fn c64_strict_fields_bodies_and_handoff_agents_refuse() {
    for (path, from, expected) in [
        (
            "prompts/synthetic.prompt.md",
            "description: Synthetic prompt\n",
            "description: missing",
        ),
        (
            "core/handoffs/synthetic.handoff.md",
            "sender: agent:core/valid\n",
            "sender: missing",
        ),
        (
            "core/handoffs/synthetic.handoff.md",
            "recipient: agent:core/valid\n",
            "recipient: missing",
        ),
        (
            "evals/synthetic.toml",
            "cases = \"eval-cases.json\"\n",
            "cases: missing",
        ),
    ] {
        assert_refused(vec![(
            "missing field",
            valid().edit(path, from, ""),
            expected,
        )]);
    }
    assert_refused(vec![
        (
            "empty prompt",
            valid().edit(
                "prompts/synthetic.prompt.md",
                "Summarize the supplied synthetic record.",
                "",
            ),
            "body: is empty",
        ),
        (
            "unknown field",
            valid().edit(
                "evals/synthetic.toml",
                "[metadata]",
                "unknown = true\n[metadata]",
            ),
            "unknown: unknown key",
        ),
        (
            "wrong agent kind",
            valid().edit(
                "core/handoffs/synthetic.handoff.md",
                "sender: agent:core/valid",
                "sender: contract:common/input",
            ),
            "sender: \"contract:common/input\" needs a qualified agent ID",
        ),
        (
            "undeclared recipient",
            valid().edit(
                "core/handoffs/synthetic.handoff.md",
                "recipient: agent:core/valid",
                "recipient: agent:core/absent",
            ),
            "recipient:",
        ),
    ]);
}

#[test]
fn eval_cases_are_exact_safe_inert_assets() {
    check_under(&valid(), &Limits::PRODUCTION).unwrap();
    for path in [
        "../outside.json",
        "/outside.json",
        "https://example.invalid/cases.json",
        "*.json",
        "a\\\\b.json",
    ] {
        assert_refused(vec![(
            "unsafe cases",
            valid().edit("evals/synthetic.toml", "eval-cases.json", path),
            "cases needs an exact owner-relative asset path",
        )]);
    }
    assert_refused(vec![(
        "non-JSON cases",
        valid().edit("evals/synthetic.toml", "eval-cases.json", "cases.txt"),
        "cases must name an exact JSON asset",
    )]);
    assert_refused(vec![(
        "missing cases",
        valid().without("evals/eval-cases.json"),
        "cannot read inventoried asset",
    )]);
    // Executable-looking input remains bytes; no evaluation or process launch.
    let tree = valid().with("evals/eval-cases.json", "[{\"command\": \"exit 99\"}]");
    check_under(&tree, &Limits::PRODUCTION).unwrap();
}

#[test]
fn c64_contracts_use_the_admitted_json_consumer() {
    assert_refused(vec![
        (
            "wrong dialect",
            valid().edit("contracts/input.schema.json", "2020-12", "2019-09"),
            "contract must declare JSON Schema 2020-12",
        ),
        (
            "wrong identity",
            valid().edit(
                "contracts/input.schema.json",
                "contract:common/input",
                "contract:common/other",
            ),
            "$id must equal contract:common/input",
        ),
    ]);
}

#[test]
fn c64_descriptors_loaded_from_data_retain_hooks() {
    let registry = from_data();
    assert_eq!(
        check_by(&valid(), &registry, &Limits::PRODUCTION),
        check_under(&valid(), &Limits::PRODUCTION),
    );
    assert_refused_by(
        &registry,
        vec![
            (
                "prompt hook",
                valid().edit(
                    "prompts/synthetic.prompt.md",
                    "supplied synthetic record",
                    "${record}",
                ),
                "unfilled template",
            ),
            (
                "handoff hook",
                valid().edit(
                    "core/handoffs/synthetic.handoff.md",
                    "## Acceptance",
                    "### Acceptance",
                ),
                "expected the sections",
            ),
            (
                "eval hook",
                contract_field("evals/synthetic.toml", "output", "contract:common/absent"),
                "must be declared in metadata.requires",
            ),
        ],
    );
}
