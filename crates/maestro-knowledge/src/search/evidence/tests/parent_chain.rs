use super::super::{
    EvidenceCounter, ExpansionMode,
    budget::{count_passages, counter_info},
    selection::{SelectionBudget, SelectionCandidate, SelectionResult, select},
};
use super::{
    selection::{CandidateSource, candidate, prepared},
    support::control,
};
use crate::search::evidence::ParentChainOrder;
use crate::search::evidence::delivery_graph::LegacyCanonicalGraph;
use std::{collections::BTreeSet, fmt::Write as _};

fn table() -> String {
    let mut markdown = String::from("# Guide\n\n| Name | Value |\n| --- | --- |\n");
    for row in 0..200 {
        writeln!(markdown, "| row-{row:03} | Unicode Ω value |").unwrap();
    }
    markdown
}

fn run(
    candidates: &[SelectionCandidate<'_>],
    units: &[BTreeSet<usize>],
    expansion: ExpansionMode,
    limit: usize,
    bytes: u32,
) -> SelectionResult {
    let counter = EvidenceCounter::Utf8Bytes;
    let info = counter_info(&counter).unwrap();
    select(
        candidates,
        units,
        &SelectionBudget {
            graph: &LegacyCanonicalGraph,
            parent_chain_order: ParentChainOrder::default(),
            expansion,
            max_passages: limit,
            max_tokens: bytes,
            counter: &counter,
            counter_info: &info,
            control: &control(),
        },
    )
    .unwrap()
}

fn parent_chain() -> ExpansionMode {
    ExpansionMode::ParentChain
}

#[test]
fn deep_row_and_header_are_exact_separate_passages() {
    let markdown = table();
    let (document, sections) = prepared(&markdown, "table.md");
    let candidates = [candidate(
        CandidateSource {
            markdown: &markdown,
            document: &document,
            sections: &sections,
        },
        "Guide",
        "row-180",
        0,
        None,
    )];
    let legacy = run(&candidates, &[], ExpansionMode::RelevantBlocks, 2, 1300);
    assert!(legacy.passages.is_empty());
    let result = run(&candidates, &[], parent_chain(), 2, 1300);
    assert_eq!(result.passages.len(), 2);
    assert_eq!(result.selected_candidates, BTreeSet::from([0]));
    assert!(result.passages[0].text.contains("| Name | Value |"));
    assert_eq!(result.passages[1].text, "| row-180 | Unicode Ω value |\n");
    for passage in &result.passages {
        assert_eq!(passage.text, markdown[passage.span.start..passage.span.end]);
        assert_eq!(passage.document_id, document.document_id);
        assert_eq!(passage.revision_id, document.revision_id);
        assert!(!passage.text.contains("row-179"));
    }
    assert!(result.passages[0].span.end < result.passages[1].span.start);
}

#[test]
fn real_serialized_cost_and_passage_limit_keep_header_and_row_atomic() {
    let markdown = table();
    let (document, sections) = prepared(&markdown, "table.md");
    let candidates = [candidate(
        CandidateSource {
            markdown: &markdown,
            document: &document,
            sections: &sections,
        },
        "Guide",
        "row-180",
        0,
        None,
    )];
    let result = run(&candidates, &[], parent_chain(), 2, 1300);
    assert_eq!(result.passages.len(), 2);
    let counter = EvidenceCounter::Utf8Bytes;
    let bytes = count_passages(
        &result.passages,
        &counter,
        &counter_info(&counter).unwrap(),
        u32::MAX,
    )
    .unwrap();
    assert_eq!(
        run(&candidates, &[], parent_chain(), 2, bytes).passages,
        result.passages
    );
    for (limit, budget) in [(2, bytes - 1), (1, 1300), (2, 500)] {
        let omitted = run(&candidates, &[], parent_chain(), limit, budget);
        assert!(omitted.passages.is_empty());
        assert!(omitted.selected_candidates.is_empty());
        assert!(omitted.omissions.evidence);
        assert_eq!(omitted.omissions.table_prefix_omissions, 1);
    }
}

#[test]
fn shared_header_does_not_cover_a_second_row_or_duplicate_the_header() {
    let markdown = table();
    let (document, sections) = prepared(&markdown, "table.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [
        candidate(source, "Guide", "row-180", 0, None),
        candidate(source, "Guide", "row-190", 1, None),
    ];
    let one = run(&candidates, &[], parent_chain(), 2, 1900);
    assert_eq!(one.selected_candidates, BTreeSet::from([0]));
    let both = run(&candidates, &[], parent_chain(), 3, 1900);
    assert_eq!(both.selected_candidates, BTreeSet::from([0, 1]));
    assert_eq!(both.passages.len(), 3);
    assert_eq!(
        both.passages
            .iter()
            .filter(|passage| passage.text.contains("| Name | Value |"))
            .count(),
        1
    );
    assert_eq!(both.parent_supports.len(), 1);
    assert_eq!(
        both.parent_supports.get(&both.passages[0].n).unwrap(),
        &["chunk-0", "chunk-1"]
    );
    for row in &both.passages[1..] {
        assert!(!both.parent_supports.contains_key(&row.n));
    }
    let conflict = run(
        &candidates,
        &[BTreeSet::from([0, 1])],
        parent_chain(),
        2,
        1900,
    );
    assert!(conflict.passages.is_empty());
    assert!(conflict.omissions.conflict);
}

#[test]
fn full_assembly_accepts_exact_parent_header_with_trace() {
    use super::super::assemble::assemble_blocking;
    use super::support::{evidence_input, fixture};
    let markdown = table();
    let fixture = fixture(&[("table.md", &markdown)]);
    let chunks = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap();
    let target = chunks
        .iter()
        .find(|chunk| {
            let span = &chunk.span;
            markdown
                .get(span.start..span.end)
                .is_some_and(|text| text.contains("row-180"))
        })
        .unwrap();
    let mut input = evidence_input(&fixture, "row-180");
    input
        .ranked
        .retain(|ranked| ranked.candidate.fused.chunk_id == target.id);
    input.evidence.expansion = parent_chain();
    input.budget.max_tokens = 5500;
    let bundle = assemble_blocking(
        &fixture.database,
        &input,
        &EvidenceCounter::Utf8Bytes,
        &control(),
    )
    .unwrap();
    assert_eq!(bundle.passages.len(), 2);
    let value = serde_json::to_value(&bundle).unwrap();
    assert_eq!(
        value["trace"][0]["parent_context_of"],
        serde_json::json!([target.id])
    );
    assert!(value["trace"][0].get("chunk_ids").is_none());
}

#[test]
fn bundle_refuses_dangling_parent_context_ids() {
    use super::super::assemble::assemble_blocking;
    use super::support::{evidence_input, fixture};
    let fixture = fixture(&[("guide.md", "# Guide\n\nPrimary seed.\n")]);
    let mut bundle = assemble_blocking(
        &fixture.database,
        &evidence_input(&fixture, "seed"),
        &EvidenceCounter::Utf8Bytes,
        &control(),
    )
    .unwrap();
    bundle.trace[0].parent_context_of = vec!["missing-seed".to_owned()];
    assert!(serde_json::to_vec(&bundle).is_err());
}

#[test]
fn orders_use_identical_seeds_but_reserve_context_differently() {
    let markdown = format!(
        "# Guide\n\nFirst seed.\n\n{}\n\n# Second\n\n{}\n",
        "Context. ".repeat(90),
        "Other. ".repeat(60)
    );
    let (document, sections) = prepared(&markdown, "orders.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [
        candidate(source, "Guide", "First seed.", 0, None),
        candidate(source, "Second", "Other.", 1, None),
    ];
    let counter = EvidenceCounter::Utf8Bytes;
    let info = counter_info(&counter).unwrap();
    let run_order = |parent_chain_order| {
        select(
            &candidates,
            &[],
            &SelectionBudget {
                expansion: parent_chain(),
                graph: &LegacyCanonicalGraph,
                parent_chain_order,
                max_passages: 2,
                max_tokens: 1500,
                counter: &counter,
                counter_info: &info,
                control: &control(),
            },
        )
        .unwrap()
    };
    let minimum = run_order(ParentChainOrder::MinimumCompleteFirst);
    let largest = run_order(ParentChainOrder::LargestFittingParent);
    assert_eq!(minimum.selected_candidates, BTreeSet::from([0, 1]));
    assert_eq!(largest.selected_candidates, BTreeSet::from([0]));
    assert_eq!(
        largest.passages,
        run_order(ParentChainOrder::LargestFittingParent).passages
    );
}

#[test]
fn canonical_graph_choices_keep_nested_procedures_and_validate_ranges() {
    use super::super::{
        delivery_graph::{ChoiceKind, DeliveryGraph},
        sections::{contains, valid_span},
    };
    let nested = concat!(
        "# Guide\n\n1. Prerequisite.\n\n   | Name | Value |\n",
        "   | --- | --- |\n   | row-180 | Ω |\n\n2. Finish.\n"
    );
    for markdown in [table(), nested.to_owned()] {
        let (document, sections) = prepared(&markdown, "graph.md");
        let candidate = candidate(
            CandidateSource {
                markdown: &markdown,
                document: &document,
                sections: &sections,
            },
            "Guide",
            "row-180",
            0,
            None,
        );
        let choices = LegacyCanonicalGraph.choices(&candidate).unwrap();
        for choice in &choices {
            assert!(!choice.ranges().is_empty());
            assert!(
                choice
                    .ranges()
                    .windows(2)
                    .all(|pair| pair[0].end < pair[1].start)
            );
            for range in &choice.ranges() {
                assert!(range.start < range.end && valid_span(*range, &markdown));
                assert!(contains(candidate.expansion.extent, *range));
            }
        }
        if markdown.contains("Prerequisite") {
            assert_eq!(choices[0].kind, ChoiceKind::Unit);
            let span = choices[0].ranges()[0];
            assert!(markdown[span.start..span.end].contains("Finish."));
        } else {
            assert_eq!(choices[0].kind, ChoiceKind::UnitWithHeader);
            assert_eq!(choices[0].ranges().len(), 2);
        }
    }
}

#[test]
fn parent_chain_honors_cancelled_and_expired_controls() {
    use super::super::EvidenceError;
    use std::{sync::atomic::Ordering, time::Instant};
    let counter = EvidenceCounter::Utf8Bytes;
    let info = counter_info(&counter).unwrap();
    for expired in [false, true] {
        let mut control = control();
        if expired {
            control.deadline = Instant::now();
        } else {
            control.cancelled.store(true, Ordering::Relaxed);
        }
        assert!(matches!(
            select(
                &[],
                &[],
                &SelectionBudget {
                    expansion: parent_chain(),
                    graph: &LegacyCanonicalGraph,
                    parent_chain_order: ParentChainOrder::default(),
                    max_passages: 2,
                    max_tokens: 1000,
                    counter: &counter,
                    counter_info: &info,
                    control: &control,
                }
            ),
            Err(EvidenceError::TimedOut)
        ));
    }
}

#[test]
fn conflict_tiers_clamp_shorter_chains_and_remain_atomic() {
    let table_markdown = table();
    let paragraph = "# Guide\n\nA conflicting independent statement.\n";
    let (table_document, table_sections) = prepared(&table_markdown, "table.md");
    let (document, sections) = prepared(paragraph, "statement.md");
    let candidates = [
        candidate(
            CandidateSource {
                markdown: &table_markdown,
                document: &table_document,
                sections: &table_sections,
            },
            "Guide",
            "row-180",
            0,
            None,
        ),
        candidate(
            CandidateSource {
                markdown: paragraph,
                document: &document,
                sections: &sections,
            },
            "Guide",
            "conflicting",
            1,
            None,
        ),
    ];
    let counter = EvidenceCounter::Utf8Bytes;
    let info = counter_info(&counter).unwrap();
    for order in [
        ParentChainOrder::MinimumCompleteFirst,
        ParentChainOrder::LargestFittingParent,
    ] {
        let run = |max_passages| {
            select(
                &candidates,
                &[BTreeSet::from([0, 1])],
                &SelectionBudget {
                    expansion: parent_chain(),
                    graph: &LegacyCanonicalGraph,
                    parent_chain_order: order,
                    max_passages,
                    max_tokens: 2400,
                    counter: &counter,
                    counter_info: &info,
                    control: &control(),
                },
            )
            .unwrap()
        };
        let fitted = run(3);
        assert_eq!(fitted.selected_candidates, BTreeSet::from([0, 1]));
        assert_eq!(fitted.passages, run(3).passages);
        let omitted = run(2);
        assert!(omitted.passages.is_empty());
        assert!(omitted.omissions.conflict);
    }
}
