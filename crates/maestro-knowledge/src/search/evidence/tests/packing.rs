use super::{CandidateSource, candidate, prepared};
use crate::search::evidence::tests::support::control;
use crate::search::evidence::{
    EvidenceCounter, EvidenceSettings, ExpansionMode,
    budget::counter_info,
    selection::{SelectionBudget, select},
};

#[test]
fn relevant_blocks_admits_another_procedure_before_expanding_first() {
    let markdown = format!(
        "# First\n\nMatched first.\n\n{}\n\n# Second\n\nMatched second.\n",
        "Background. ".repeat(70)
    );
    let (document, sections) = prepared(&markdown, "packing.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [
        candidate(source, "First", "Matched first.", 0, None),
        candidate(source, "Second", "Matched second.", 1, None),
    ];
    let counter = EvidenceCounter::AnswerBoundUtf8Bytes;
    let info = counter_info(&counter).unwrap();
    let control = control();
    let run = |expansion| {
        select(
            &candidates,
            &[],
            &SelectionBudget {
                expansion,
                max_passages: 2,
                max_tokens: 1000,
                counter: &counter,
                counter_info: &info,
                control: &control,
                reserved: &[],
            },
        )
        .unwrap()
    };
    assert_eq!(run(ExpansionMode::FullSection).passages.len(), 1);
    let packed = run(ExpansionMode::RelevantBlocks);
    assert_eq!(packed.passages.len(), 2);
    for passage in packed.passages {
        assert_eq!(passage.text, markdown[passage.span.start..passage.span.end]);
        assert!(passage.text.contains("Matched"));
    }
}

#[test]
fn named_counter_defaults_preserve_legacy_and_exact_refuses() {
    let defaults: EvidenceSettings = serde_json::from_str("{}").unwrap();
    assert_eq!(defaults.expansion, ExpansionMode::FullSection);
    assert!(matches!(
        defaults.counter().unwrap(),
        EvidenceCounter::Utf8Bytes
    ));
    let exact: EvidenceSettings = serde_json::from_str(r#"{"evidence_counter":"exact"}"#).unwrap();
    assert!(
        exact
            .counter()
            .unwrap_err()
            .to_string()
            .contains("resolved answerer's tokenizer must be qualified")
    );
}

#[test]
fn relevant_table_prefix_keeps_header_and_whole_row_with_exact_span() {
    let markdown = format!(
        "# Values\n\n| Name | Value |\n| --- | --- |\n| matched | yes |\n| later | {} |\n",
        "unrelated ".repeat(100)
    );
    let (document, sections) = prepared(&markdown, "rows.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [candidate(source, "Values", "matched", 0, None)];
    let counter = EvidenceCounter::AnswerBoundUtf8Bytes;
    let info = counter_info(&counter).unwrap();
    let control = control();
    let packed = select(
        &candidates,
        &[],
        &SelectionBudget {
            expansion: ExpansionMode::RelevantBlocks,
            max_passages: 2,
            max_tokens: 300,
            counter: &counter,
            counter_info: &info,
            control: &control,
            reserved: &[],
        },
    )
    .unwrap();
    assert_eq!(packed.passages.len(), 1);
    let passage = &packed.passages[0];
    assert!(passage.text.contains("| Name | Value |"));
    assert!(passage.text.contains("| matched | yes |"));
    assert!(!passage.text.contains("later"));
    assert_eq!(passage.text, markdown[passage.span.start..passage.span.end]);
}

#[test]
fn over_budget_table_prefix_falls_back_without_truncating_a_row() {
    let markdown =
        "# Values\n\n| Name | Value |\n| --- | --- |\n| matched | yes |\n| later | no |\n";
    let (document, sections) = prepared(markdown, "fallback.md");
    let source = CandidateSource {
        markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [candidate(source, "Values", "matched", 0, None)];
    let counter = EvidenceCounter::AnswerBoundUtf8Bytes;
    let info = counter_info(&counter).unwrap();
    let control = control();
    let packed = select(
        &candidates,
        &[],
        &SelectionBudget {
            expansion: ExpansionMode::RelevantBlocks,
            max_passages: 2,
            max_tokens: 1,
            counter: &counter,
            counter_info: &info,
            control: &control,
            reserved: &[],
        },
    )
    .unwrap();
    assert_eq!(packed.omissions.table_prefix_fallbacks, 1);
    assert!(packed.omissions.evidence);
    assert!(packed.passages.is_empty());
}

#[test]
fn relevant_blocks_keep_complete_list_steps_and_nested_tables() {
    let markdown = concat!(
        "# Procedure\n\n1. Prerequisite.\n\n   | Name | Value |\n",
        "   | --- | --- |\n   | matched | yes |\n   | later | no |\n",
        "\n2. Finish the operation.\n"
    );
    let (document, sections) = prepared(markdown, "procedure.md");
    let source = CandidateSource {
        markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [candidate(source, "Procedure", "matched", 0, None)];
    let counter = EvidenceCounter::AnswerBoundUtf8Bytes;
    let info = counter_info(&counter).unwrap();
    let control = control();
    let packed = select(
        &candidates,
        &[],
        &SelectionBudget {
            expansion: ExpansionMode::RelevantBlocks,
            max_passages: 1,
            max_tokens: 1000,
            counter: &counter,
            counter_info: &info,
            control: &control,
            reserved: &[],
        },
    )
    .unwrap();
    assert_eq!(packed.passages.len(), 1);
    assert!(packed.passages[0].text.contains("Prerequisite."));
    assert!(packed.passages[0].text.contains("Finish the operation."));
    assert!(packed.passages[0].text.contains("| later | no |"));
}

#[test]
fn relevant_blocks_spend_leftover_budget_on_neighbors_in_rank_order() {
    let markdown = format!(
        "# Alpha\n\n{}\n\nMatched alpha.\n\n# Bravo\n\n{}\n\nMatched bravo.\n",
        "Alpha context. ".repeat(10),
        "Bravo context. ".repeat(10)
    );
    let (document, sections) = prepared(&markdown, "order.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [
        candidate(source, "Alpha", "Matched alpha.", 1, None),
        candidate(source, "Bravo", "Matched bravo.", 0, None),
    ];
    let counter = EvidenceCounter::AnswerBoundUtf8Bytes;
    let info = counter_info(&counter).unwrap();
    let control = control();
    let packed = select(
        &candidates,
        &[],
        &SelectionBudget {
            expansion: ExpansionMode::RelevantBlocks,
            max_passages: 2,
            max_tokens: 310,
            counter: &counter,
            counter_info: &info,
            control: &control,
            reserved: &[],
        },
    )
    .unwrap();
    let texts = packed
        .passages
        .iter()
        .map(|passage| passage.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert!(texts.contains(&"Matched alpha.\n"), "{texts:?}");
    assert!(
        texts
            .iter()
            .any(|text| text.contains("Bravo context.") && text.contains("Matched bravo.")),
        "{texts:?}"
    );
}
