use super::{CandidateSource, candidate, prepared, run_selection};
use std::collections::BTreeSet;

#[test]
fn conflict_units_are_omitted_atomically_when_the_passage_limit_is_one() {
    let markdown_a = concat!(
        "# Guide\n\n| Entity | Attribute | Value |\n",
        "| --- | --- | --- |\n| Agent | Port | 7005 |\n"
    );
    let markdown_b = markdown_a.replace("7005", "7006");
    let (document_a, sections_a) = prepared(markdown_a, "conflict-a.md");
    let (document_b, sections_b) = prepared(&markdown_b, "conflict-b.md");
    let source_a = CandidateSource {
        markdown: markdown_a,
        document: &document_a,
        sections: &sections_a,
    };
    let source_b = CandidateSource {
        markdown: &markdown_b,
        document: &document_b,
        sections: &sections_b,
    };
    let candidates = [
        candidate(source_a, "Guide", "7005", 0, None),
        candidate(source_b, "Guide", "7006", 1, None),
    ];

    let result = run_selection(&candidates, &[BTreeSet::from([0, 1])], 1, u32::MAX);

    assert!(result.passages.is_empty());
    assert!(result.omissions.evidence);
    assert!(result.omissions.conflict);
}
