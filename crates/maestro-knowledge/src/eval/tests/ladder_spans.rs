//! Citation locality against the expected section's pinned source extent.

use super::ladder::{count, passing};
use crate::eval::{Floor, SectionRef, score_ladder};

fn citation(document: &str, revision: &str, span: [usize; 2]) -> SectionRef {
    SectionRef {
        document_id: document.to_owned(),
        revision_id: Some(revision.to_owned()),
        chunk_id: Some("chunk".to_owned()),
        section_id: None,
        span: Some(span),
    }
}

fn scored(expected: SectionRef, citation: SectionRef) -> usize {
    let mut rows = passing();
    rows[0].expected = vec![expected];
    rows[0].ask = super::ladder::answered(vec![citation], 0);
    let score = score_ladder(&super::ladder::suite(), &rows);
    count(&score, Floor::Answered).0
}

/// The section `section` of the document `right` at `revision`, over the
/// bytes 10 to 20.
fn expected_section() -> SectionRef {
    let mut expected = SectionRef::section("right", "section");
    expected.revision_id = Some("revision".to_owned());
    expected.span = Some([10, 20]);
    expected
}

#[test]
fn an_overlapping_span_of_the_pinned_revision_matches() {
    assert_eq!(
        scored(expected_section(), citation("right", "revision", [15, 25])),
        84
    );
}

#[test]
fn spans_touching_the_extent_do_not_match() {
    assert_eq!(
        scored(expected_section(), citation("right", "revision", [0, 10])),
        83
    );
    assert_eq!(
        scored(expected_section(), citation("right", "revision", [20, 25])),
        83
    );
}

#[test]
fn an_overlapping_span_of_another_revision_does_not_match() {
    assert_eq!(
        scored(expected_section(), citation("right", "other", [15, 18])),
        83
    );
}

#[test]
fn an_overlapping_span_of_another_document_does_not_match() {
    assert_eq!(
        scored(expected_section(), citation("other", "revision", [15, 18])),
        83
    );
}

#[test]
fn a_sibling_section_citation_with_a_disjoint_span_does_not_match() {
    let mut sibling = citation("right", "revision", [25, 30]);
    sibling.section_id = Some("sibling".to_owned());

    assert_eq!(scored(expected_section(), sibling), 83);
}

#[test]
fn an_overlapping_ancestor_section_citation_matches_by_span() {
    let mut expected = SectionRef::section("right", "child");
    expected.revision_id = Some("revision".to_owned());
    expected.span = Some([10, 20]);
    let mut ancestor = citation("right", "revision", [12, 18]);
    ancestor.section_id = Some("parent".to_owned());

    assert_eq!(scored(expected, ancestor), 84);
}

#[test]
fn disjoint_spans_with_a_gap_do_not_match() {
    let expected = expected_section();

    assert_eq!(
        scored(expected.clone(), citation("right", "revision", [0, 5])),
        83
    );
    assert_eq!(
        scored(expected, citation("right", "revision", [25, 30])),
        83
    );
}

#[test]
fn a_sectionless_citation_can_match_by_overlapping_span() {
    let expected = expected_section();

    assert_eq!(
        scored(expected, citation("right", "revision", [12, 13])),
        84
    );
}

#[test]
fn whole_document_expectation_keeps_matching_any_citation_of_its_document() {
    let mut expected = SectionRef::document("right");
    expected.revision_id = Some("revision".to_owned());
    assert_eq!(scored(expected, citation("right", "revision", [0, 1])), 84);
}

#[test]
fn whole_document_expectation_refuses_a_citation_of_another_revision() {
    let mut expected = SectionRef::document("right");
    expected.revision_id = Some("revision".to_owned());
    assert_eq!(scored(expected, citation("right", "other", [0, 1])), 83);
}
