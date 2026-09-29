//! Tests of the delivery score: which anchors deliver a section, how much of
//! it they cover, the components a composed answer needs, and the rows that
//! earn no credit.

use super::ladder::{REFUSED, answered, ask_ending, ranked_at};
use crate::{
    eval::{AskOutcome, DeliveryScore, LadderQuestion, SearchOutcome, SectionRef, score_delivery},
    search::evidence::Anchor,
    suite::Suite,
};

/// A suite of the answerable questions `ids`, then one unanswerable `u0`.
fn suite(ids: &[&str]) -> Suite {
    ids.iter()
        .map(|id| (*id, true))
        .chain([("u0", false)])
        .map(|(id, answerable)| {
            let expected = if answerable {
                serde_json::json!([{ "source_ref": "doc.md", "heading_path": [] }])
            } else {
                serde_json::json!([])
            };
            serde_json::json!({
                "schema": "maestro-suite/1",
                "id": id,
                "language": "en",
                "question": format!("question {id}"),
                "answerable": answerable,
                "expected": expected,
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .parse()
        .expect("the tests' suite parses")
}

/// The section `section` of `doc` in `rev`, over the bytes 10 to 20.
fn section(section: &str) -> SectionRef {
    let mut expected = SectionRef::section("doc", section);
    expected.revision_id = Some("rev".to_owned());
    expected.span = Some([10, 20]);
    expected
}

/// `section`, as a part of the answer named `component`.
fn part(section_id: &str, component: &str) -> SectionRef {
    SectionRef {
        component: Some(component.to_owned()),
        ..section(section_id)
    }
}

/// An anchor of `doc` in `rev` over `span`, in no section.
fn anchor(span: [usize; 2]) -> Anchor {
    Anchor {
        source_ref: "doc.md".to_owned(),
        doc_id: "doc".to_owned(),
        revision_id: "rev".to_owned(),
        section_id: None,
        span,
        digest: "sha256:0".to_owned(),
    }
}

/// The row of `id`, ranked, expecting `expected`, whose evidence holds
/// `delivered`; it did not ask.
fn row(id: &str, expected: Vec<SectionRef>, delivered: Vec<Anchor>) -> LadderQuestion {
    LadderQuestion {
        id: id.to_owned(),
        expected,
        search: ranked_at(Some(1)),
        ask: ask_ending(AskOutcome::Failed),
        delivered,
    }
}

/// The score of one question `a` expecting `expected`, whose evidence holds
/// `delivered`, when the configuration did not ask.
fn one(expected: Vec<SectionRef>, delivered: Vec<Anchor>) -> DeliveryScore {
    score_delivery(&suite(&["a"]), &[row("a", expected, delivered)], false)
}

#[test]
fn an_anchor_naming_the_section_delivers_it_whole_only_if_it_covers_it() {
    let named = Anchor {
        section_id: Some("s1".to_owned()),
        ..anchor([0, 5])
    };
    let score = one(vec![section("s1")], vec![named]);

    assert_eq!(
        score,
        DeliveryScore {
            answerable: 1,
            delivered: 1,
            fully_delivered: 0,
            median_coverage_permille: Some(0),
            composition_cases: 0,
            compositions_delivered: 0,
            uncredited: 0,
        }
    );
}

#[test]
fn another_document_revision_or_a_touching_span_delivers_nothing() {
    let other_document = Anchor {
        doc_id: "other".to_owned(),
        ..anchor([10, 20])
    };
    let other_revision = Anchor {
        revision_id: "old".to_owned(),
        ..anchor([10, 20])
    };
    for anchors in [
        vec![other_document],
        vec![other_revision],
        vec![anchor([20, 30]), anchor([0, 10])],
    ] {
        let score = one(vec![section("s1")], anchors);
        assert_eq!(
            (score.delivered, score.fully_delivered),
            (0, 0),
            "{score:?}"
        );
        assert_eq!(score.median_coverage_permille, Some(0));
    }
}

#[test]
fn a_heading_line_overlap_delivers_a_section_but_not_whole() {
    let score = one(vec![section("s1")], vec![anchor([5, 11])]);

    assert_eq!((score.delivered, score.fully_delivered), (1, 0));
    assert_eq!(score.median_coverage_permille, Some(100));
    assert!(score.to_markdown().contains("median coverage 10.0%"));
}

#[test]
fn the_union_of_overlapping_anchors_covers_a_section_whole() {
    let adjacent = one(vec![section("s1")], vec![anchor([15, 25]), anchor([0, 15])]);
    let overlapping = one(
        vec![section("s1")],
        vec![anchor([10, 16]), anchor([12, 20])],
    );
    let nested = one(
        vec![section("s1")],
        vec![anchor([11, 19]), anchor([12, 13])],
    );

    assert_eq!(adjacent.fully_delivered, 1);
    assert_eq!(overlapping.fully_delivered, 1);
    assert_eq!(overlapping.median_coverage_permille, Some(1000));
    assert_eq!(nested.fully_delivered, 0);
    assert_eq!(nested.median_coverage_permille, Some(800));
}

#[test]
fn a_question_takes_its_best_section_and_the_median_is_nearest_rank() {
    let ids = ["a", "b", "c", "d"];
    let rows = [
        row("a", vec![section("s1")], vec![anchor([10, 13])]),
        row(
            "b",
            vec![SectionRef::section("elsewhere", "s9"), section("s1")],
            vec![anchor([10, 15])],
        ),
        row("c", vec![section("s1")], vec![anchor([10, 20])]),
        row("d", vec![section("s1")], Vec::new()),
    ];
    let score = score_delivery(&suite(&ids), &rows, false);

    assert_eq!((score.answerable, score.delivered), (4, 3));
    assert_eq!(score.fully_delivered, 1);
    assert_eq!(score.median_coverage_permille, Some(300));
}

#[test]
fn a_document_expected_whole_is_covered_by_any_anchor_of_its_revision() {
    let mut whole = SectionRef::document("doc");
    whole.revision_id = Some("rev".to_owned());
    let score = one(vec![whole.clone()], vec![anchor([500, 501])]);
    let missed = one(vec![whole], Vec::new());

    assert_eq!((score.delivered, score.fully_delivered), (1, 1));
    assert_eq!((missed.delivered, missed.fully_delivered), (0, 0));
}

#[test]
fn a_degraded_search_or_a_failed_ask_is_credited_with_nothing() {
    let ids = ["a", "b", "c", "d"];
    let delivered = || vec![anchor([10, 20])];
    let mut failed_search = row("a", vec![section("s1")], delivered());
    failed_search.search.outcome = SearchOutcome::Failed;
    let mut timed_out = row("b", vec![section("s1")], delivered());
    timed_out.ask = ask_ending(AskOutcome::TimedOut);
    let mut refused = row("c", vec![section("s1")], delivered());
    refused.ask = ask_ending(REFUSED);
    let mut answered_row = row("d", vec![section("s1")], delivered());
    answered_row.ask = answered(Vec::new(), 0);
    let rows = [failed_search, timed_out, refused, answered_row];

    let asked = score_delivery(&suite(&ids), &rows, true);
    let unasked = score_delivery(&suite(&ids), &rows, false);

    assert_eq!((asked.delivered, asked.uncredited), (2, 2));
    assert_eq!(asked.median_coverage_permille, Some(0));
    assert_eq!((unasked.delivered, unasked.uncredited), (3, 1));
}

#[test]
fn rows_pair_with_questions_by_id_and_unanswerable_ones_are_skipped() {
    let rows = [
        row("u0", vec![section("s1")], vec![anchor([10, 20])]),
        row("stray", vec![section("s1")], vec![anchor([10, 20])]),
        row("b", vec![section("s1")], vec![anchor([10, 20])]),
    ];
    let score = score_delivery(&suite(&["a", "b"]), &rows, false);

    assert_eq!(score.answerable, 2);
    assert_eq!((score.delivered, score.uncredited), (1, 1));
}

#[test]
fn a_composed_answer_needs_every_component() {
    let expected = || {
        vec![
            part("condition-a", "condition"),
            SectionRef {
                span: Some([30, 40]),
                ..part("condition-b", "condition")
            },
            SectionRef {
                span: Some([50, 60]),
                ..part("action", "action")
            },
        ]
    };
    let both = one(expected(), vec![anchor([35, 36]), anchor([55, 56])]);
    let condition_only = one(expected(), vec![anchor([10, 20]), anchor([30, 40])]);

    assert_eq!(
        (both.composition_cases, both.compositions_delivered),
        (1, 1)
    );
    assert_eq!(
        (
            condition_only.composition_cases,
            condition_only.compositions_delivered
        ),
        (1, 0)
    );
    assert!(
        both.to_markdown()
            .contains("Required-composition coverage: 1/1")
    );
}

#[test]
fn copies_and_ungrouped_names_without_components_are_no_composition() {
    let one_component = one(
        vec![part("s1", "answer"), part("s2", "answer")],
        vec![anchor([10, 20])],
    );
    let unnamed = one(vec![section("s1"), section("s2")], vec![anchor([10, 20])]);

    assert_eq!(one_component.composition_cases, 0);
    assert_eq!(unnamed.composition_cases, 0);
}

#[test]
fn the_markdown_states_each_count_and_its_rule() {
    let score = DeliveryScore {
        answerable: 14,
        delivered: 8,
        fully_delivered: 5,
        median_coverage_permille: Some(875),
        composition_cases: 1,
        compositions_delivered: 0,
        uncredited: 2,
    };
    let markdown = score.to_markdown();

    assert!(markdown.contains("- Delivered-section recall: 8/14 (same document"));
    assert!(markdown.contains("- Fully delivered sections: 5/14 ("));
    assert!(markdown.contains("median coverage 87.5%\n"));
    assert!(markdown.contains("- Required-composition coverage: 0/1\n"));
    assert!(markdown.contains("- Uncredited questions: 2 ("));
    assert!(
        DeliveryScore::default()
            .to_markdown()
            .contains("median coverage none")
    );
}
