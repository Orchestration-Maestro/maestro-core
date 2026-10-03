//! Approved manifest handoff document guards; not runtime qualification.

use super::graph_fixture::workspace;
use std::{fs, path::Path};

/// G01's bounded document surface; no other slice's implementation is inspected.
const CONTRACT_DOCUMENTS: &[&str] = &[
    "specs/002-knowledge-graph/spec.md",
    "specs/002-knowledge-graph/plan.md",
    "specs/002-knowledge-graph/tasks.md",
    "docs/adr/0021-embedded-ladybug-graph-projection.md",
    "docs/adr/README.md",
    "docs/adr/0004-neo4j-for-the-graph-projection.md",
    "docs/adr/0020-rust-libraries-with-named-dependency-exceptions.md",
    "docs/architecture/README.md",
    "docs/architecture/01-knowledge-pipeline.md",
    "docs/architecture/02-retrieval-and-knowledge-graph.md",
    "docs/architecture/04-intelligence-backend.md",
    "docs/architecture/05-platform-and-operations.md",
    "docs/architecture/06-roadmap.md",
    "docs/architecture/08-traceability.md",
];

/// Only each document's designated handoff can supply contract evidence.
fn handoff(text: &str, document: usize) -> &str {
    let (start, end) = match document {
        0 => (
            "\n### Approved manifest v4 cross-slice obligations\n",
            "\n## Needs owner action\n",
        ),
        1 => (
            "\n#### Manifest v4 settings and lock handoff\n",
            "\n### A4 Reads and complete proofs\n",
        ),
        2 => (
            "\n**Manifest delta contract:**",
            "\nG01's frozen public rule/oracle",
        ),
        3 => (
            "\n## Manifest v4 settings and lock handoff\n",
            "\n## Qualification and safety conditions\n",
        ),
        _ => return "",
    };
    text.split_once(start)
        .and_then(|(_, rest)| rest.split_once(end))
        .map_or("", |(section, _)| section)
}

/// Clause boundaries keep a plural lock inventory from proving the singular lock contract.
fn contains_clause(section: &str, required: &str) -> bool {
    section.match_indices(required).any(|(start, _)| {
        !section[start + required.len()..].starts_with(|ch: char| ch.is_ascii_alphanumeric())
    })
}

/// Require a Markdown link target, not a local heading or a plain-text anchor.
fn has_handoff_link(text: &str) -> bool {
    text.split("](")
        .skip(1)
        .filter_map(|link| link.split_once(')'))
        .any(|(target, _)| {
            target
                .strip_suffix("plan.md#manifest-v4-settings-and-lock-handoff")
                .is_some_and(|prefix| prefix.is_empty() || prefix.ends_with('/'))
        })
}

#[test]
fn s2_traceability_references_use_the_s2_reconciliation_section() {
    let text =
        fs::read_to_string(workspace().join("docs/architecture/08-traceability.md")).unwrap();
    assert!(text.contains("## 24. S2"), "the S2 reconciliation heading");
    for key in [
        "owner.n020",
        "rag.N016 claims",
        "rag.N052 layers",
        "rag.N023 roles",
        "rag.N023 access",
        "rag.N023 variants",
        "rag.N023 editions",
        "rag.N016 methods",
    ] {
        let prefix = format!("| {key} |");
        let row = text.lines().find(|line| line.starts_with(&prefix)).unwrap();
        assert!(row.ends_with("§24 |"), "{key}: {row}");
    }
}

#[test]
fn graph_fixture_bounds_match_approved_s3_d14() {
    let plan = fs::read_to_string(workspace().join(CONTRACT_DOCUMENTS[1])).unwrap();
    for (text, approved) in [
        (plan.clone(), true),
        (
            plan.replace(
                "| Setting | Minimum | Maximum | Default | Constraint |",
                "| Setting | Default | Maximum | Minimum | Constraint |",
            ),
            false,
        ),
    ] {
        let rows: Vec<_> = handoff(&text, 1)
            .lines()
            .skip_while(|line| !line.starts_with("| Setting |"))
            .take_while(|line| line.starts_with('|'))
            .collect();
        // Independent literals from S3 30b702b plan D14, not native optima.
        assert_eq!(
            rows == [
                "| Setting | Minimum | Maximum | Default | Constraint |",
                "| --- | --- | --- | --- | --- |",
                "| `graphdb.buffer_pool_size` | 16 MiB | 1 GiB | 256 MiB | No zero/auto |",
                concat!(
                    "| `graphdb.max_db_size` | 16 MiB | 1 TiB | 16 GiB | ",
                    "Power of two; not a disk quota |"
                ),
                "| `graphdb.max_num_threads` | 1 | 64 | 2 | Not Cargo parallelism |",
            ],
            approved,
            "D14 table columns and bounds"
        );
    }
}

#[test]
fn graph_fixture_setting_and_backend_type_keep_registry_and_lock_handoff() {
    for (document, path) in CONTRACT_DOCUMENTS[..4].iter().enumerate() {
        let text = fs::read_to_string(workspace().join(path)).unwrap();
        let section = handoff(&text, document);
        for (candidate, approved) in [
            (text.clone(), true),
            (text.replace("one producer per", "two producers per"), false),
            (
                text.replace(
                    "core/backends/graphdb/config.toml",
                    "core/backends/ladybug/config.toml",
                ),
                false,
            ),
            (text.replace("registry", "duplicate defaults"), false),
            (
                text.replace("complete non-resource lock", "partial non-resource lock"),
                false,
            ),
            (text.replace("registered", "duplicate"), false),
            (format!("{}\n{section}", text.replace(section, "")), false),
        ] {
            let section = handoff(&candidate, document)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let agrees = [
                "`graph.engine`",
                "`type = \"ladybug\"`",
                "registered adapter",
                "`core/backends/graphdb/config.toml`",
                "`settings/defaults.toml`",
                "registry",
                "one producer per key",
                "C46",
                "C47a",
                "C48",
                "OA1",
                "complete non-resource lock",
            ]
            .into_iter()
            .all(|required| contains_clause(&section, required));
            assert_eq!(agrees, approved, "{path}: handoff agreement");
        }
    }
}

#[test]
fn graph_fixture_documents_link_one_handoff_and_reject_retired_engine_resources() {
    for path in CONTRACT_DOCUMENTS.iter().copied().chain([
        "tests/fixtures/synthetic/graph/defaults.md",
        "tests/fixtures/synthetic/graph/defaults.json",
    ]) {
        let text = fs::read_to_string(workspace().join(path)).unwrap();
        for (candidate, approved) in [
            (text.clone(), true),
            (format!("{text}\nSelect `engine:graphdb`."), false),
            (format!("{text}\nSelect `engine:ladybug`."), false),
            (format!("{text}\nSelect `engine:`."), false),
            (format!("{text}\n**Crawler engine:** candidate."), true),
            (format!("{text}\nSelect `engines/x/y`."), false),
        ] {
            let identifier = candidate.split("engine:").skip(1).any(|tail| {
                tail.starts_with(|ch: char| ch.is_ascii_lowercase() || ch.is_ascii_digit())
            });
            let current =
                !identifier && !candidate.contains("`engine:`") && !candidate.contains("engines/");
            assert_eq!(current, approved, "{path}: retired resources");
        }
        if Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            && !path.starts_with("tests/fixtures/")
            && path != CONTRACT_DOCUMENTS[1]
        {
            for (candidate, linked) in [
                (text.clone(), true),
                (
                    text.replace(
                        "plan.md#manifest-v4-settings-and-lock-handoff",
                        "plain-text-handoff",
                    ),
                    false,
                ),
            ] {
                assert_eq!(has_handoff_link(&candidate), linked, "{path}: handoff link");
            }
        }
    }
}
