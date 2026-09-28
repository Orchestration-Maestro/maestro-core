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

#[test]
fn citation_span_intersection_is_strict_and_pinned() {
    let mut expected = SectionRef::section("right", "section");
    expected.revision_id = Some("revision".to_owned());
    expected.span = Some([10, 20]);

    assert_eq!(
        scored(expected.clone(), citation("right", "revision", [15, 25])),
        84
    );
    assert_eq!(
        scored(expected.clone(), citation("right", "revision", [0, 10])),
        83
    );
    assert_eq!(
        scored(expected.clone(), citation("right", "revision", [20, 25])),
        83
    );
    assert_eq!(
        scored(expected.clone(), citation("right", "other", [15, 18])),
        83
    );
    assert_eq!(
        scored(expected, citation("other", "revision", [15, 18])),
        83
    );
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
    let mut expected = SectionRef::section("right", "section");
    expected.revision_id = Some("revision".to_owned());
    expected.span = Some([10, 20]);

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
    let mut expected = SectionRef::section("right", "section");
    expected.revision_id = Some("revision".to_owned());
    expected.span = Some([10, 20]);

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
