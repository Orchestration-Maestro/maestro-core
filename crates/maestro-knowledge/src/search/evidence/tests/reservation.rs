//! Each question part's best passage is admitted before any other, within
//! the budget and the answer-bound wire ceiling.

use super::{CandidateSource, candidate, count_passages, prepared};
use crate::search::evidence::tests::support::control;
use crate::search::evidence::{
    EvidenceCounter, ExpansionMode,
    budget::counter_info,
    selection::{SelectionBudget, SelectionResult, select},
};

/// Three sections apart, each long enough to fill a small budget alone.
fn markdown() -> String {
    let background = "Background. ".repeat(70);
    format!(
        "# First\n\nMatched first.\n\n{background}\n\n# Second\n\nMatched second.\n\n\
         {background}\n\n# Third\n\nMatched third.\n\n{background}\n"
    )
}

/// Selects the three sections of [`markdown`], ranked in order, under
/// `expansion`, `counter`, `max_passages` and `max_tokens`, reserving the
/// chunks `reserved`.
fn run(
    expansion: ExpansionMode,
    counter: &EvidenceCounter,
    (max_passages, max_tokens): (usize, u32),
    reserved: &[String],
) -> (SelectionResult, u32) {
    let markdown = markdown();
    let (document, sections) = prepared(&markdown, "reserve.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [
        candidate(source, "First", "Matched first.", 0, None),
        candidate(source, "Second", "Matched second.", 1, None),
        candidate(source, "Third", "Matched third.", 2, None),
    ];
    let info = counter_info(counter).unwrap();
    let control = control();
    let result = select(
        &candidates,
        &[],
        &SelectionBudget {
            expansion,
            max_passages,
            max_tokens,
            counter,
            counter_info: &info,
            control: &control,
            reserved,
        },
    )
    .unwrap();
    let used = count_passages(&result.passages, counter, &info, max_tokens).unwrap();
    (result, used)
}

/// Whether `result` delivers the section whose marker is `marker`.
fn delivers(result: &SelectionResult, marker: &str) -> bool {
    result
        .passages
        .iter()
        .any(|passage| passage.text.contains(marker))
}

/// The chunk IDs of the candidates at `positions`.
fn chunks(positions: &[usize]) -> Vec<String> {
    positions
        .iter()
        .map(|position| format!("chunk-{position}"))
        .collect()
}

#[test]
fn a_reserved_passage_is_admitted_before_a_better_ranked_one_fills_the_budget() {
    let counter = EvidenceCounter::AnswerBoundUtf8Bytes;
    let (whole, _) = run(ExpansionMode::FullSection, &counter, (3, 1000), &[]);
    assert!(delivers(&whole, "Matched first."));
    assert!(!delivers(&whole, "Matched third."));
    let (parts, used) = run(
        ExpansionMode::FullSection,
        &counter,
        (3, 1000),
        &chunks(&[2]),
    );
    assert!(delivers(&parts, "Matched third."));
    assert!(used <= 1000);
    let (both, used) = run(
        ExpansionMode::FullSection,
        &counter,
        (3, 1000),
        &chunks(&[0, 2]),
    );
    assert!(delivers(&both, "Matched first."));
    assert!(delivers(&both, "Matched third."));
    assert!(used <= 1000);
}

#[test]
fn every_part_keeps_a_passage_before_any_part_widens() {
    let counter = EvidenceCounter::AnswerBoundUtf8Bytes;
    for expansion in [ExpansionMode::FullSection, ExpansionMode::RelevantBlocks] {
        let (result, used) = run(expansion, &counter, (3, 700), &chunks(&[0, 2]));
        assert!(delivers(&result, "Matched first."), "{expansion:?}");
        assert!(delivers(&result, "Matched third."), "{expansion:?}");
        assert!(used <= 700, "{expansion:?}");
        for passage in &result.passages {
            assert_eq!(
                passage.text,
                markdown()[passage.span.start..passage.span.end]
            );
        }
    }
}

#[test]
fn a_reserved_passage_never_exceeds_the_passage_or_byte_budget() {
    let counter = EvidenceCounter::Utf8Bytes;
    let (one, _) = run(
        ExpansionMode::FullSection,
        &counter,
        (1, 6000),
        &chunks(&[2, 0]),
    );
    assert_eq!(one.passages.len(), 1);
    assert!(delivers(&one, "Matched third."));
    assert!(!delivers(&one, "Matched first."));
    assert!(one.omissions.evidence);
    let (tight, used) = run(
        ExpansionMode::FullSection,
        &counter,
        (3, 600),
        &chunks(&[2, 0]),
    );
    assert!(delivers(&tight, "Matched third."));
    assert!(!delivers(&tight, "Matched first."));
    assert!(tight.omissions.evidence);
    assert!(used <= 600);
}

#[test]
fn nothing_reserved_or_an_unknown_chunk_selects_as_before() {
    let counter = EvidenceCounter::AnswerBoundUtf8Bytes;
    for expansion in [ExpansionMode::FullSection, ExpansionMode::RelevantBlocks] {
        let (before, _) = run(expansion, &counter, (3, 1000), &[]);
        let (unknown, _) = run(expansion, &counter, (3, 1000), &chunks(&[9]));
        assert_eq!(before.passages, unknown.passages);
        assert_eq!(before.selected_candidates, unknown.selected_candidates);
    }
}
